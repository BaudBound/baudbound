FROM node:24-alpine AS base
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable
WORKDIR /workspace/apps/editor

FROM base AS deps
COPY apps/editor/package.json apps/editor/pnpm-lock.yaml apps/editor/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

FROM deps AS build
COPY apps/editor/ ./
COPY schemas/package-limits.json /workspace/schemas/package-limits.json
RUN pnpm build

FROM node:24-alpine AS runner
ENV NODE_ENV="production"
ENV HOSTNAME="0.0.0.0"
ENV PORT="3000"
WORKDIR /app

RUN addgroup --system --gid 1001 nodejs && adduser --system --uid 1001 nextjs

COPY --from=build --chown=nextjs:nodejs /workspace/apps/editor/.next/standalone ./
COPY --from=build --chown=nextjs:nodejs /workspace/apps/editor/.next/static ./apps/editor/.next/static
COPY --from=build --chown=nextjs:nodejs /workspace/apps/editor/public ./apps/editor/public
RUN test -f apps/editor/server.js && test -d apps/editor/.next/static && test -d apps/editor/public

USER nextjs

EXPOSE 3000
CMD ["node", "apps/editor/server.js"]
