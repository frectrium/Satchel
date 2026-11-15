output "site_url" {
  description = "The Satchel website."
  value       = "https://${aws_cloudfront_distribution.site.domain_name}"
}

output "distribution_id" {
  description = "For cache invalidations after a deploy."
  value       = aws_cloudfront_distribution.site.id
}

output "site_bucket" {
  value = aws_s3_bucket.site.id
}

output "files_bucket" {
  value = aws_s3_bucket.files.id
}

output "backend_instance_id" {
  description = "For Session Manager and to start it again after the budget stops it."
  value       = aws_instance.backend.id
}
