# Storage nodes spread evenly over the three zones, and one control instance
# for the frontend, Block Index, verifier (with the Master), Prometheus and
# Grafana, as in exercise plan section 3. Instance types follow DDL-015. Every
# instance runs the arm64 image from #34, pulled by commit SHA.

data "aws_ssm_parameter" "al2023_arm64" {
  name = "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64"
}

locals {
  # node-z1-1, node-z2-1, node-z3-1, node-z1-2, ... so consecutive nodes
  # land in different zones.
  nodes = [
    for i in range(var.node_count) : {
      id   = "node-z${i % 3 + 1}-${floor(i / 3) + 1}"
      zone = "z${i % 3 + 1}"
      az   = i % 3
    }
  ]
}

resource "aws_instance" "node" {
  count = var.node_count

  ami                    = data.aws_ssm_parameter.al2023_arm64.value
  instance_type          = var.node_instance_type
  subnet_id              = aws_subnet.zone[local.nodes[count.index].az].id
  vpc_security_group_ids = [aws_security_group.cluster.id]
  iam_instance_profile   = aws_iam_instance_profile.instance.name

  root_block_device {
    volume_type = "gp3"
    volume_size = var.node_volume_gib
    encrypted   = true
  }

  metadata_options {
    http_tokens = "required"
  }

  user_data = templatefile("${path.module}/templates/node.sh.tftpl", {
    image = var.image
    zone  = local.nodes[count.index].zone
  })
  user_data_replace_on_change = true

  tags = {
    Name         = local.nodes[count.index].id
    satchel-role = "node"
    satchel-zone = local.nodes[count.index].zone
  }
}

locals {
  satchel_nodes = join(",", [
    for i, n in local.nodes : "${n.id}@${n.zone}=http://${aws_instance.node[i].private_ip}:8090"
  ])
}

resource "aws_instance" "control" {
  ami                    = data.aws_ssm_parameter.al2023_arm64.value
  instance_type          = var.control_instance_type
  subnet_id              = aws_subnet.zone[0].id
  vpc_security_group_ids = [aws_security_group.cluster.id, aws_security_group.operator.id]
  iam_instance_profile   = aws_iam_instance_profile.instance.name

  root_block_device {
    volume_type = "gp3"
    volume_size = 30
    encrypted   = true
  }

  metadata_options {
    http_tokens = "required"
  }

  user_data = templatefile("${path.module}/templates/control.sh.tftpl", {
    image         = var.image
    git_ref       = var.git_ref
    satchel_nodes = local.satchel_nodes
    encode        = var.encode
    repair_after  = var.repair_after
    node_targets = {
      for z in ["z1", "z2", "z3"] : z => [
        for i, n in local.nodes : "${aws_instance.node[i].private_ip}:8090" if n.zone == z
      ]
    }
  })
  user_data_replace_on_change = true

  tags = {
    Name         = "satchel-control"
    satchel-role = "control"
  }
}
