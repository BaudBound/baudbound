import assert from "node:assert/strict";
import test from "node:test";

import { WikiJsClient } from "../src/graphql-client.mjs";

test("uses the HTTPS GraphQL endpoint and bearer token", async () => {
  let request;
  const client = new WikiJsClient({
    baseUrl: "https://wiki.example.test/docs",
    token: "secret-token",
    fetchImpl: async (url, options) => {
      request = { options, url };
      return jsonResponse({ data: { pages: { list: [] } } });
    },
  });

  assert.deepEqual(await client.listPages(), []);
  assert.equal(request.url, "https://wiki.example.test/docs/graphql");
  assert.equal(request.options.headers.authorization, "Bearer secret-token");
  assert.equal(JSON.parse(request.options.body).variables.constructor, Object);
});

test("rejects insecure endpoints and failed Wiki.js mutations", async () => {
  assert.throws(
    () => new WikiJsClient({ baseUrl: "http://wiki.example.test", token: "token" }),
    /must use HTTPS/,
  );

  const client = new WikiJsClient({
    baseUrl: "https://wiki.example.test",
    token: "token",
    fetchImpl: async () =>
      jsonResponse({
        data: {
          pages: {
            create: {
              responseResult: { message: "permission denied", succeeded: false },
            },
          },
        },
      }),
  });
  await assert.rejects(client.createPage(page()), /permission denied/);
});

function page() {
  return {
    content: "# Home\n",
    description: "Home",
    editor: "markdown",
    isPrivate: false,
    isPublished: true,
    locale: "en",
    path: "home",
    tags: ["managed-by-git"],
    title: "Home",
  };
}

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    headers: { "content-type": "application/json" },
    status,
  });
}
