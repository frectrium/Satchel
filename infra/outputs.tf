output "api_url" {
  description = "Satchel frontend: PUT, GET and DELETE /blocks/{hash}."
  value       = "http://${aws_instance.control.public_ip}:8080"
}

output "calculator_url" {
  description = "Pricing calculator page (#54), served by the frontend."
  value       = "http://${aws_instance.control.public_ip}:8080/calculator"
}

output "grafana_url" {
  description = "Grafana with the Satchel overview dashboard (#21)."
  value       = "http://${aws_instance.control.public_ip}:3000"
}

output "control_instance_id" {
  description = "For Session Manager: aws ssm start-session --target <id>."
  value       = aws_instance.control.id
}

output "nodes" {
  description = "Storage nodes by Satchel ID, with instance ID, zone and private IP. Stop one with aws ec2 stop-instances for the failure demo."
  value = {
    for i, n in local.nodes : n.id => {
      instance_id = aws_instance.node[i].id
      az          = aws_instance.node[i].availability_zone
      private_ip  = aws_instance.node[i].private_ip
    }
  }
}
