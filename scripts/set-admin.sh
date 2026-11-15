#!/usr/bin/env bash
# Sets the website's admin login. Asks for the password without echoing it,
# hashes it with Argon2id and stores only "name:hash" in the SecureString
# parameter /satchel/admins, then restarts the backend so it takes effect.
# The password itself is never written anywhere.
#
#   scripts/set-admin.sh [name]     (default name: admin)
set -euo pipefail
cd "$(dirname "$0")/.."
name=${1:-admin}
[[ "$name" =~ ^[A-Za-z0-9_.-]+$ ]] || { echo "use letters, digits, dot, dash or underscore for the name" >&2; exit 1; }

cargo build -q --release -p satchel-files
read -rsp "Password for $name: " pw; echo
read -rsp "Same password again: " again; echo
[[ "$pw" == "$again" ]] || { echo "the passwords don't match" >&2; exit 1; }
hash=$(printf '%s\n' "$pw" | "${CARGO_TARGET_DIR:-target}/release/satchel-files" hash-password)
unset pw again

aws ssm put-parameter --region ap-south-1 --name /satchel/admins --type SecureString \
    --value "$name:$hash" --overwrite >/dev/null
echo "Stored the login for $name."

if instance=$(terraform -chdir=infra/web output -raw backend_instance_id 2>/dev/null); then
    if aws ssm send-command --region ap-south-1 --instance-ids "$instance" --document-name AWS-RunShellScript \
        --parameters 'commands=["systemctl restart satchel-files"]' >/dev/null; then
        echo "Restarted the backend; the login works in a few seconds."
    else
        echo "Couldn't restart the backend now. It reads the login the next time it starts."
    fi
fi
