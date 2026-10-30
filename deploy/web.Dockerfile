# The built client behind nginx; the ingress sends /api and /ws to the server.
FROM node:24-alpine AS build
RUN npm install --global pnpm@12
WORKDIR /src/web
COPY web/package.json web/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY web ./
RUN pnpm build

FROM nginxinc/nginx-unprivileged:alpine
COPY deploy/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /src/web/dist /usr/share/nginx/html
EXPOSE 8080
