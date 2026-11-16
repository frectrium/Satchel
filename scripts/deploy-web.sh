#!/usr/bin/env bash
# Deploys the website and the S3-mode files API to the stack in infra/web:
# syncs the static site, uploads the backend binary, restarts the service
# and clears CloudFront's cache.
#
#   scripts/deploy-web.sh <path to a Linux arm64 satchel-files binary>
#
# Uses the AWS CLI's current credentials (for example AWS_PROFILE) in ap-south-1.
set -euo pipefail
cd "$(dirname "$0")/.."

binary=${1:?usage: scripts/deploy-web.sh <satchel-files binary for linux arm64>}
[[ -f "$binary" ]] || { echo "no such file: $binary" >&2; exit 1; }
export AWS_REGION=ap-south-1

out() { terraform -chdir=infra/web output -raw "$1"; }
site_bucket=$(out site_bucket)
files_bucket=$(out files_bucket)
distribution=$(out distribution_id)
instance=$(out backend_instance_id)

echo "--- site"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
scripts/stage-site.sh "$stage/site" >/dev/null
aws s3 sync "$stage/site" "s3://$site_bucket" --delete --cache-control "max-age=60" --only-show-errors
aws s3 cp "$stage/site/calculator/pkg/satchel_calc_bg.wasm" "s3://$site_bucket/calculator/pkg/satchel_calc_bg.wasm" \
    --content-type application/wasm --cache-control "max-age=60" --only-show-errors

echo "--- backend binary"
aws s3 cp "$binary" "s3://$files_bucket/deploy/satchel-files" --only-show-errors
state=$(aws ec2 describe-instances --instance-ids "$instance" --query 'Reservations[0].Instances[0].State.Name' --output text)
if [[ "$state" == running ]]; then
    aws ssm send-command --instance-ids "$instance" --document-name AWS-RunShellScript \
        --parameters 'commands=["systemctl restart satchel-files"]' --query 'Command.CommandId' --output text >/dev/null \
        || echo "restart through Session Manager failed; the service picks up the binary on its next start"
else
    echo "backend instance is $state; it picks up the binary when it starts"
fi

echo "--- cache"
aws cloudfront create-invalidation --distribution-id "$distribution" --paths '/*' --query 'Invalidation.Id' --output text >/dev/null
echo "Deployed: $(out site_url)"
