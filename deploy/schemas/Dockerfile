FROM nginx:1.29-alpine

COPY deploy/schemas/nginx.conf /etc/nginx/conf.d/default.conf
COPY schemas/*.schema.json /usr/share/nginx/html/
COPY schemas/nodes/ /usr/share/nginx/html/nodes/

RUN find /usr/share/nginx/html -type d -exec chmod 0555 {} \; \
	&& find /usr/share/nginx/html -type f -name "*.json" -exec chmod 0444 {} \;

EXPOSE 80
