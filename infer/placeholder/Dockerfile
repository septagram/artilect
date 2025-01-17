FROM nginx:alpine

RUN apk add --no-cache gettext

COPY config/nginx.conf.template /etc/nginx/nginx.conf.template
COPY config/init-nginx-conf.sh /docker-entrypoint.d/

RUN chmod +x /docker-entrypoint.d/init-nginx-conf.sh 