# Every Satchel instance can reach every other one. From outside, only the
# operator's addresses reach the control instance, on the API (which also
# serves the calculator page) and Grafana. There is no SSH: Session Manager
# gives a shell when one is needed, and the demo stops nodes with the EC2 API.

resource "aws_security_group" "cluster" {
  name        = "satchel-cluster"
  description = "Satchel instances talk to each other"
  vpc_id      = aws_vpc.satchel.id

  tags = { Name = "satchel-cluster" }
}

resource "aws_vpc_security_group_ingress_rule" "cluster_internal" {
  security_group_id            = aws_security_group.cluster.id
  referenced_security_group_id = aws_security_group.cluster.id
  ip_protocol                  = "-1"
  description                  = "Any traffic between Satchel instances"
}

resource "aws_vpc_security_group_egress_rule" "cluster_out" {
  security_group_id = aws_security_group.cluster.id
  cidr_ipv4         = "0.0.0.0/0"
  ip_protocol       = "-1"
  description       = "Image pulls, package installs and Session Manager"
}

resource "aws_security_group" "operator" {
  name        = "satchel-operator"
  description = "The operator reaches the API and Grafana"
  vpc_id      = aws_vpc.satchel.id

  tags = { Name = "satchel-operator" }
}

resource "aws_vpc_security_group_ingress_rule" "operator" {
  for_each = {
    for pair in setproduct(var.operator_cidrs, [8080, 3000]) :
    "${pair[0]}-${pair[1]}" => { cidr = pair[0], port = pair[1] }
  }

  security_group_id = aws_security_group.operator.id
  cidr_ipv4         = each.value.cidr
  ip_protocol       = "tcp"
  from_port         = each.value.port
  to_port           = each.value.port
  description       = each.value.port == 8080 ? "Satchel API and calculator" : "Grafana"
}

# Session Manager instead of SSH keys: no key pairs and no open port 22.
resource "aws_iam_role" "instance" {
  name = "satchel-instance"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy_attachment" "ssm" {
  role       = aws_iam_role.instance.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_instance_profile" "instance" {
  name = "satchel-instance"
  role = aws_iam_role.instance.name
}
