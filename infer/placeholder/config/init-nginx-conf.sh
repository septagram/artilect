#!/bin/sh

# Try to get VirtualBox host IP
VBOX_IP=$(ip route show | grep default | awk '{print $3}')

if [ -n "$VBOX_IP" ]; then
  export PROXY_UPSTREAM_ADDR=${PROXY_UPSTREAM_ADDR:-$VBOX_IP}
  export PROXY_UPSTREAM_PORT=${PROXY_UPSTREAM_PORT:-11000}
else
  export PROXY_UPSTREAM_ADDR=${PROXY_UPSTREAM_ADDR:-api.openai.com}
  export PROXY_UPSTREAM_PORT=${PROXY_UPSTREAM_PORT:-443}
fi

envsubst '${PROXY_UPSTREAM_ADDR} ${PROXY_UPSTREAM_PORT}' < /etc/nginx/nginx.conf.template > /etc/nginx/nginx.conf

exec nginx -g 'daemon off;' 