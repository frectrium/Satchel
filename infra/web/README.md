# Satchel website on AWS

The always-on part of the demo (#63): the website and the S3-mode files API.
It's separate from the 3-AZ Satchel cluster in `infra/`, which only comes up
for the viva hour.

| File | What it creates |
|---|---|
| `storage.tf` | Two private buckets: the static site (read by CloudFront only) and the files admins upload |
| `backend.tf` | One `t4g.small` running `satchel-files` with `SATCHEL_BACKEND=s3` in a private subnet: no public IP, no internet route, S3 through a gateway endpoint and Session Manager through interface endpoints |
| `cdn.tf` | One CloudFront distribution: the site from S3, and `/api/*` to the backend through a CloudFront VPC origin, so that hop stays on AWS's network. HTTPS on a `*.cloudfront.net` address |
| `budget.tf` | A monthly budget counted before credits: email at 80%, and the backend instance stopped automatically at 100% |

Uploads to S3 are limited to 100 MiB per file and 2 GiB in total, and only
admins can upload, list or delete. Anyone can use the calculator and a local
Satchel install. At most two password checks run at once, and after three
wrong passwords for a user name each attempt has to wait, doubling up to a
minute.

**The budget covers the whole account, but stops only this stack's backend.**
If the viva cluster in `infra/` is up at the same time, its cost counts toward
the $50 and it keeps running past it. Tear it down with `terraform destroy`
straight after each use.

## Admin login

The site has one admin login for S3 uploads. Set its password yourself; the
script asks for it without echoing, hashes it with Argon2id and stores only
the hash in the SecureString parameter `/satchel/admins`:

```bash
scripts/set-admin.sh          # user name "admin"; pass another name as the argument
```

It restarts the backend so the new login works straight away.

## Bring it up

```bash
cd infra/web
terraform init
terraform plan -out=web.tfplan -var "alert_email=<your email>"
terraform apply web.tfplan
cd ../..
scripts/deploy-web.sh <satchel-files built for aarch64-unknown-linux-musl>
```

CloudFront takes a few minutes to deploy. `terraform -chdir=infra/web output
site_url` prints the address. CloudFront picks the `dxxxx.cloudfront.net`
name itself; a custom domain needs a certificate and a DNS record.

## When the budget stops the backend

The site stays up (S3 and CloudFront cost almost nothing at this size), but
uploads fail until the instance runs again:

```bash
aws ec2 start-instances --region ap-south-1 --instance-ids "$(terraform -chdir=infra/web output -raw backend_instance_id)"
```

## Tear it down

```bash
terraform -chdir=infra/web destroy -var "alert_email=<your email>"
```

Both buckets are emptied and deleted with it, including every uploaded file.
