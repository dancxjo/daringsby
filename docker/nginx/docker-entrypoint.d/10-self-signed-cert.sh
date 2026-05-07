#!/bin/sh
set -eu

cert_dir="${PETE_TLS_CERT_DIR:-/etc/nginx/self-signed}"
cert="${PETE_TLS_CERT:-$cert_dir/localhost.crt}"
key="${PETE_TLS_KEY:-$cert_dir/localhost.key}"
common_name="${PETE_TLS_COMMON_NAME:-localhost}"
days="${PETE_TLS_DAYS:-825}"
config="$cert_dir/openssl.cnf"

mkdir -p "$cert_dir"

if [ -s "$cert" ] && [ -s "$key" ]; then
    exit 0
fi

cat > "$config" <<EOF
[req]
default_bits = 2048
prompt = no
default_md = sha256
distinguished_name = dn
x509_extensions = v3_req

[dn]
CN = $common_name

[v3_req]
subjectAltName = @alt_names
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth

[alt_names]
DNS.1 = localhost
DNS.2 = $common_name
IP.1 = 127.0.0.1
IP.2 = ::1
EOF

openssl req \
    -x509 \
    -nodes \
    -newkey rsa:2048 \
    -days "$days" \
    -keyout "$key" \
    -out "$cert" \
    -config "$config"
