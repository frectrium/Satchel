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
