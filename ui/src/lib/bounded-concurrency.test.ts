import { describe, expect, it } from "vitest";

import { runWithConcurrency } from "@/lib/bounded-concurrency";

describe("runWithConcurrency", () => {
	it("never exceeds the requested concurrency", async () => {
		let active = 0;
		let maximum = 0;
		const completed: number[] = [];

		await runWithConcurrency([1, 2, 3, 4, 5], 2, async (value) => {
			active += 1;
			maximum = Math.max(maximum, active);
			await new Promise((resolve) => setTimeout(resolve, 2));
			completed.push(value);
			active -= 1;
		});

		expect(maximum).toBe(2);
		expect(completed.sort()).toEqual([1, 2, 3, 4, 5]);
	});

	it("does not start queued work after cancellation", async () => {
		let cancelled = false;
		const started: number[] = [];

		await runWithConcurrency(
			[1, 2, 3],
			1,
			async (value) => {
				started.push(value);
				cancelled = true;
			},
			() => cancelled,
		);

		expect(started).toEqual([1]);
	});
});
