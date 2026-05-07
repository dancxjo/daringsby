FROM nginx:1.27-alpine

RUN apk add --no-cache openssl

COPY default.conf.template /etc/nginx/templates/default.conf.template
COPY docker-entrypoint.d/10-self-signed-cert.sh /docker-entrypoint.d/10-self-signed-cert.sh

RUN chmod +x /docker-entrypoint.d/10-self-signed-cert.sh
