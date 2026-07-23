export async function runWithConcurrency<T>(
	items: T[],
	limit: number,
	worker: (item: T) => Promise<void>,
	cancelled: () => boolean = () => false,
) {
	if (!Number.isInteger(limit) || limit < 1) {
		throw new Error("Concurrency limit must be a positive integer.");
	}
	let index = 0;
	const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
		while (index < items.length && !cancelled()) {
			const item = items[index++];
			await worker(item);
		}
	});
	await Promise.all(workers);
}
