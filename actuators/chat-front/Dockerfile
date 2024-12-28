FROM rust:1.83 as builder

# Install wasm-pack
RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /usr/src/chat-front
COPY . .

# Build the application
RUN wasm-pack build --target web

FROM nginx:alpine
COPY --from=builder /usr/src/chat-front/dist /usr/share/nginx/html
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]