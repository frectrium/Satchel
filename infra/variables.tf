variable "aws_region" {
  description = "AWS region for every Satchel resource."
  type        = string
  default     = "ap-south-1"

  validation {
    condition     = var.aws_region == "ap-south-1"
    error_message = "Satchel must be deployed in ap-south-1."
  }
}

variable "project_name" {
  description = "Value of the mandatory project cost-allocation tag."
  type        = string
  default     = "satchel"

  validation {
    condition     = var.project_name == "satchel"
    error_message = "Satchel resources must use project=satchel."
  }
}

variable "image" {
  description = "Satchel image to run, pulled by commit SHA or release tag, for example ghcr.io/frectrium/satchel:<sha> (#34)."
  type        = string

  validation {
    condition     = can(regex("^ghcr\\.io/frectrium/satchel:[A-Za-z0-9._-]+$", var.image)) && !endswith(var.image, ":latest")
    error_message = "Use ghcr.io/frectrium/satchel with a commit SHA or release tag, not latest, so a deployment records what it ran."
  }
}

variable "git_ref" {
  description = "Commit or tag of this repository for the Grafana dashboards. Use the same commit as the image."
  type        = string
}

variable "operator_cidrs" {
  description = "Addresses allowed to reach the API (8080) and Grafana (3000), for example [\"203.0.113.7/32\"]. Pass with -var, never commit."
  type        = list(string)

  validation {
    condition     = length(var.operator_cidrs) > 0 && alltrue([for c in var.operator_cidrs : can(cidrnetmask(c)) && c != "0.0.0.0/0"])
    error_message = "Give at least one CIDR, and not 0.0.0.0/0."
  }
}

variable "node_count" {
  description = "Storage nodes, a multiple of 3 so each zone gets the same number: 9 for the demo, 12 or 18 for E2 (exercise plan, section 3)."
  type        = number
  default     = 9

  validation {
    condition     = var.node_count >= 9 && var.node_count % 3 == 0
    error_message = "node_count must be a multiple of 3 and at least 9, for RS(6,3) with three fragments per zone (DDL-007)."
  }
}

variable "node_instance_type" {
  description = "Storage node instance type (DDL-015)."
  type        = string
  default     = "m7g.medium"
}

variable "control_instance_type" {
  description = "Instance type for the frontend, Block Index, verifier, Prometheus and Grafana (DDL-015)."
  type        = string
  default     = "m7g.large"
}

variable "node_volume_gib" {
  description = "gp3 volume per storage node, in GiB (exercise plan, section 3)."
  type        = number
  default     = 50
}

variable "encode" {
  description = "\"on\" lets the Master erasure-code closed volumes (DDL-008). Leave \"off\" until the frontend reads coded volumes (#70)."
  type        = string
  default     = "off"

  validation {
    condition     = contains(["on", "off"], var.encode)
    error_message = "encode must be \"on\" or \"off\"."
  }
}

variable "repair_after" {
  description = "How long a node can be down before repair starts: 15 minutes in Magic Pocket [B2], 30 s for the demo (DDL-010)."
  type        = string
  default     = "30s"
}
