# build rust
FROM rust:slim AS engine-builder
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /engine
COPY ./engine .
# build web assembly package
RUN wasm-pack build --target web --out-dir /frontend-src/wasm

# build frontend
FROM node:20-slim AS frontend-builder
WORKDIR /app
COPY ./frontend/package*.json ./
RUN npm ci

# xopy everything from frontend
COPY ./frontend .
# copy compiled wasm into the source folder
COPY --from=engine-builder /frontend-src/wasm ./src/wasm

RUN npm run build

# production serve 
FROM nginx:alpine AS production
COPY --from=frontend-builder /app/dist /usr/share/nginx/html
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
