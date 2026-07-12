import { NextResponse } from "next/server";

export function proxy() {
	const response = NextResponse.next();
	response.headers.set("Content-Security-Policy", "frame-ancestors 'self';");

	return response;
}
