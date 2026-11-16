# The files API in S3 mode on one small instance in a private subnet of the
# default VPC: no public IP and no route to the internet. CloudFront reaches
# it through a VPC origin (cdn.tf), so the login, the session cookie and
# uploads never cross the internet unencrypted. S3 is reached through a
# gateway endpoint, and Session Manager and the parameter store through
# interface endpoints.

data "aws_vpc" "default" {
  default = true
}

resource "aws_subnet" "private" {
  vpc_id                  = data.aws_vpc.default.id
  availability_zone       = "ap-south-1a"
  cidr_block              = "172.31.200.0/24"
  map_public_ip_on_launch = false

  tags = { Name = "satchel-web-private" }
}

# No route to the internet gateway: only the endpoints below are reachable.
resource "aws_route_table" "private" {
  vpc_id = data.aws_vpc.default.id
  tags   = { Name = "satchel-web-private" }
}

resource "aws_route_table_association" "private" {
  subnet_id      = aws_subnet.private.id
  route_table_id = aws_route_table.private.id
}

resource "aws_vpc_endpoint" "s3" {
  vpc_id            = data.aws_vpc.default.id
  service_name      = "com.amazonaws.ap-south-1.s3"
  vpc_endpoint_type = "Gateway"
  route_table_ids   = [aws_route_table.private.id]
  tags              = { Name = "satchel-web-s3" }
}

resource "aws_security_group" "endpoints" {
  name        = "satchel-web-endpoints"
  description = "HTTPS from the backend to the SSM endpoints"
  vpc_id      = data.aws_vpc.default.id
}

resource "aws_vpc_security_group_ingress_rule" "endpoints_from_backend" {
  security_group_id            = aws_security_group.endpoints.id
  referenced_security_group_id = aws_security_group.backend.id
  ip_protocol                  = "tcp"
  from_port                    = 443
  to_port                      = 443
  description                  = "Backend to SSM"
}

# Session Manager (ssm, ssmmessages, ec2messages) and the admin parameter (ssm).
resource "aws_vpc_endpoint" "ssm" {
  for_each = toset(["ssm", "ssmmessages", "ec2messages"])

  vpc_id              = data.aws_vpc.default.id
  service_name        = "com.amazonaws.ap-south-1.${each.key}"
  vpc_endpoint_type   = "Interface"
  subnet_ids          = [aws_subnet.private.id]
  security_group_ids  = [aws_security_group.endpoints.id]
  private_dns_enabled = true
  tags                = { Name = "satchel-web-${each.key}" }
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

# CloudFront creates this group in the VPC for its VPC origins.
data "aws_security_group" "cloudfront_vpc_origins" {
  vpc_id = data.aws_vpc.default.id
  name   = "CloudFront-VPCOrigins-Service-SG"

  depends_on = [aws_cloudfront_vpc_origin.backend]
}

resource "aws_vpc_security_group_ingress_rule" "from_vpc_origin" {
  security_group_id            = aws_security_group.backend.id
  referenced_security_group_id = data.aws_security_group.cloudfront_vpc_origins.id
  ip_protocol                  = "tcp"
  from_port                    = 47800
  to_port                      = 47800
  description                  = "CloudFront VPC origin to the files API"
}

resource "aws_vpc_security_group_egress_rule" "out" {
  security_group_id = aws_security_group.backend.id
  cidr_ipv4         = "0.0.0.0/0"
  ip_protocol       = "-1"
  description       = "S3 and SSM through the VPC endpoints"
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
  ami                         = data.aws_ssm_parameter.al2023_arm64.value
  instance_type               = var.backend_instance_type
  subnet_id                   = aws_subnet.private.id
  associate_public_ip_address = false
  vpc_security_group_ids      = [aws_security_group.backend.id]
  iam_instance_profile        = aws_iam_instance_profile.backend.name

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

  # The boot script fetches the binary from S3 and the admins from SSM.
  depends_on = [aws_vpc_endpoint.s3, aws_vpc_endpoint.ssm, aws_route_table_association.private]
}
