import { NextResponse } from "next/server";

// Runs per-request in the live server process (unlike next.config.ts's headers(),
// which is resolved once at build time), so this reflects the container's actual
// runtime UMAMI_SRC without requiring an image rebuild.
export function proxy() {
	const response = NextResponse.next();

	const umamiOrigin = process.env.UMAMI_SRC ? new URL(process.env.UMAMI_SRC).origin : "";
	response.headers.set("Content-Security-Policy", `frame-ancestors 'self'${umamiOrigin ? ` ${umamiOrigin}` : ""};`);

	return response;
}
