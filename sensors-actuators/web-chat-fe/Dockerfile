FROM rust:1.83 as builder

# Install wasm-pack
RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /usr/src/web-chat-fe
COPY . .

# Build the application
RUN wasm-pack build --target web

FROM nginx:alpine
COPY --from=builder /usr/src/web-chat-fe/dist /usr/share/nginx/html
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]