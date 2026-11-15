# One VPC with a public subnet in each of the three ap-south-1 zones, which
# stand in for Magic Pocket's zones (DDL-005). Public subnets with an internet
# gateway let the instances pull the image without a NAT gateway, which would
# cost more than the instances for a short demo (our inference). The security
# groups in security.tf decide what is reachable.

data "aws_availability_zones" "available" {
  state = "available"

  filter {
    name   = "zone-name"
    values = ["ap-south-1a", "ap-south-1b", "ap-south-1c"]
  }
}

locals {
  zones = sort(data.aws_availability_zones.available.names)
}

resource "aws_vpc" "satchel" {
  cidr_block           = "10.42.0.0/16"
  enable_dns_support   = true
  enable_dns_hostnames = true

  tags = { Name = "satchel" }
}

resource "aws_internet_gateway" "satchel" {
  vpc_id = aws_vpc.satchel.id

  tags = { Name = "satchel" }
}

resource "aws_subnet" "zone" {
  count = 3

  vpc_id                  = aws_vpc.satchel.id
  availability_zone       = local.zones[count.index]
  cidr_block              = "10.42.${count.index + 1}.0/24"
  map_public_ip_on_launch = true

  tags = { Name = "satchel-z${count.index + 1}" }
}

resource "aws_route_table" "public" {
  vpc_id = aws_vpc.satchel.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.satchel.id
  }

  tags = { Name = "satchel-public" }
}

resource "aws_route_table_association" "zone" {
  count = 3

  subnet_id      = aws_subnet.zone[count.index].id
  route_table_id = aws_route_table.public.id
}
