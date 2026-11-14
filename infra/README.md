# AWS infrastructure

Issue #33 prepares the AWS and Terraform workflow for the Satchel cluster in
issue #20. This directory deliberately contains no cluster resources yet.

## Fixed project settings

| Setting | Value |
|---|---|
| AWS region | `ap-south-1` (Mumbai) |
| Terraform | `1.17.2` |
| AWS provider | `6.73.0` |
| Required cost tag | `project=satchel` |
| State for the course deployment | Local, ignored by Git |

Terraform applies the project tag through the AWS provider's `default_tags`, so
resources added in issue #20 inherit it. Keep an explicit `project` tag on any
resource type that does not support provider default tags.

## Team prerequisites

The team must agree these values outside the repository before the first plan:

- the AWS account to use;
- IAM Identity Center (SSO) permission-set names, or IAM user names if SSO is
  unavailable;
- the monthly budget amount and alert email address.

Do not put account IDs, email addresses, access keys, SSO cache files or session
tokens in this repository. Require MFA for every human identity. Prefer IAM
Identity Center because it issues short-lived CLI credentials. If IAM users are
the only option, create no access keys until a CLI profile is needed and rotate
or remove them after the project.

The account administrator should give the three team members only the access
needed for the Terraform resources agreed in issue #20. Record the account and
permission-set choice in the private team channel, not in Git.

## Configure the AWS CLI

Each member creates a named profile locally. For IAM Identity Center:

```bash
aws configure sso --profile satchel
aws sso login --profile satchel
AWS_PROFILE=satchel AWS_REGION=ap-south-1 aws sts get-caller-identity
```

For an IAM user, run `aws configure --profile satchel`, enter credentials only
at the prompt, and verify the same `sts get-caller-identity` command. Never pass
keys as command arguments or write them to a Terraform variable file.

Before continuing, all three members must see the agreed account from
`get-caller-identity` and the region must be `ap-south-1`.

## Budget and cost-allocation tag

An account administrator must complete both items in AWS Billing before the
cluster is deployed:

1. Activate the user-defined `project` cost-allocation tag.
2. Create a monthly cost budget filtered to `project=satchel`, using the amount
   and email address agreed by the team.

Do not commit the budget amount or alert email. Save a screenshot of the active
budget in the private team workspace, then record completion in issue #33.

## Why state is local

The course deployment is temporary and has one operator at a time. Local state
avoids creating a permanent S3 state bucket and lock table before the cluster
exists. The operator who runs `terraform apply` also runs `terraform destroy`
from the same checkout before handing control to somebody else.

This workflow has a tradeoff: state is not shared or recoverable from Git. Do
not run two plans at once, do not delete the local state while resources exist,
and do not send the state file to another person because state can contain
sensitive values. If the team needs concurrent operators or a long-lived
environment, replace the local backend with an encrypted, versioned S3 backend
with locking in a separate reviewed PR.

## Initialize and check

Terraform uses the standard AWS credential chain, so select the named profile
in the shell rather than putting it in provider configuration:

```bash
cd infra
AWS_PROFILE=satchel terraform init
terraform fmt -check -recursive
AWS_PROFILE=satchel terraform validate
```

CI runs formatting and validation without credentials. It disables backend
initialization and does not plan or apply resources.

## Deployment safety

Before every apply:

- confirm the named profile with `aws sts get-caller-identity`;
- confirm the budget alert and `project` cost-allocation tag are active;
- run `terraform fmt -check -recursive` and `terraform validate`;
- run `terraform plan -out=satchel.tfplan` and review every create, update and
  delete;
- tell the team that the environment is going up and name the operator;
- check that every planned resource is in `ap-south-1` and carries
  `project=satchel`.

Never run `terraform apply` from CI. Apply only a reviewed saved plan.

## Teardown checklist

The same operator tears down the environment after each experiment run:

- [ ] Tell the team that teardown is starting and stop active benchmarks.
- [ ] Save required metrics and experiment output under the documented results
      workflow. Do not save credentials or state.
- [ ] Run `AWS_PROFILE=satchel terraform plan -destroy` and review the targets.
- [ ] Run `AWS_PROFILE=satchel terraform destroy`.
- [ ] Confirm `terraform show` reports no managed resources.
- [ ] Check the AWS console for resources Terraform could not remove, including
      load balancers, EBS volumes, snapshots, Elastic IPs and log groups.
- [ ] Confirm no Satchel EC2 instances are running in `ap-south-1`.
- [ ] Tell the team teardown is complete and keep the budget alert active.

Do not delete `terraform.tfstate` until both Terraform and the AWS console show
that the deployment is gone.
