# The files API in S3 mode on one small instance in the default VPC. Only
# CloudFront reaches it, on port 47800. An Elastic IP keeps its DNS name
# stable when the budget stops and someone starts it again.

data "aws_vpc" "default" {
  default = true
}

data "aws_subnets" "default" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
  filter {
    name   = "availability-zone"
    values = ["ap-south-1a"]
  }
}

data "aws_ec2_managed_prefix_list" "cloudfront" {
  name = "com.amazonaws.global.cloudfront.origin-facing"
}

data "aws_ssm_parameter" "al2023_arm64" {
  name = "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64"
}

resource "aws_security_group" "backend" {
  name        = "satchel-web-backend"
  description = "Only CloudFront reaches the files API"
  vpc_id      = data.aws_vpc.default.id
}

resource "aws_vpc_security_group_ingress_rule" "from_cloudfront" {
  security_group_id = aws_security_group.backend.id
  prefix_list_id    = data.aws_ec2_managed_prefix_list.cloudfront.id
  ip_protocol       = "tcp"
  from_port         = 47800
  to_port           = 47800
  description       = "CloudFront to the files API"
}

resource "aws_vpc_security_group_egress_rule" "out" {
  security_group_id = aws_security_group.backend.id
  cidr_ipv4         = "0.0.0.0/0"
  ip_protocol       = "-1"
  description       = "S3, SSM and package installs"
}

resource "aws_iam_role" "backend" {
  name = "satchel-web-backend"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy" "backend" {
  name = "satchel-web-backend"
  role = aws_iam_role.backend.id
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid      = "Files"
        Effect   = "Allow"
        Action   = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject"]
        Resource = "${aws_s3_bucket.files.arn}/files/*"
      },
      {
        Sid       = "ListFiles"
        Effect    = "Allow"
        Action    = "s3:ListBucket"
        Resource  = aws_s3_bucket.files.arn
        Condition = { StringLike = { "s3:prefix" = ["files/*"] } }
      },
      {
        Sid      = "Binary"
        Effect   = "Allow"
        Action   = "s3:GetObject"
        Resource = "${aws_s3_bucket.files.arn}/deploy/*"
      },
      {
        Sid      = "Admins"
        Effect   = "Allow"
        Action   = "ssm:GetParameter"
        Resource = "arn:aws:ssm:ap-south-1:${data.aws_caller_identity.me.account_id}:parameter/satchel/admins"
      },
    ]
  })
}

resource "aws_iam_role_policy_attachment" "backend_ssm" {
  role       = aws_iam_role.backend.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_instance_profile" "backend" {
  name = "satchel-web-backend"
  role = aws_iam_role.backend.name
}

resource "aws_instance" "backend" {
  ami                    = data.aws_ssm_parameter.al2023_arm64.value
  instance_type          = var.backend_instance_type
  subnet_id              = data.aws_subnets.default.ids[0]
  vpc_security_group_ids = [aws_security_group.backend.id]
  iam_instance_profile   = aws_iam_instance_profile.backend.name

  root_block_device {
    volume_type = "gp3"
    volume_size = 8
    encrypted   = true
  }

  metadata_options {
    http_tokens = "required"
  }

  user_data = templatefile("${path.module}/templates/backend.sh.tftpl", {
    files_bucket    = aws_s3_bucket.files.id
    max_file_bytes  = var.max_file_bytes
    max_total_bytes = var.max_total_bytes
  })
  user_data_replace_on_change = true

  tags = { Name = "satchel-web-backend" }
}

resource "aws_eip" "backend" {
  domain = "vpc"
  tags   = { Name = "satchel-web-backend" }
}

resource "aws_eip_association" "backend" {
  instance_id   = aws_instance.backend.id
  allocation_id = aws_eip.backend.id
}
