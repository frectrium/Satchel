variable "alert_email" {
  description = "Where the budget sends its alerts. Pass with -var, never commit."
  type        = string
  sensitive   = true
}

variable "budget_usd" {
  description = "Monthly cost, before credits, at which the backend instance is stopped."
  type        = number
  default     = 50
}

variable "backend_instance_type" {
  description = "Instance type for the files API in S3 mode. A few admins upload at most 2 GiB, so a small burstable type is enough (our inference)."
  type        = string
  default     = "t4g.small"
}

variable "max_file_bytes" {
  description = "Largest upload to S3: 100 MiB."
  type        = number
  default     = 104857600
}

variable "max_total_bytes" {
  description = "Most the S3 bucket may hold: 2 GiB."
  type        = number
  default     = 2147483648
}
