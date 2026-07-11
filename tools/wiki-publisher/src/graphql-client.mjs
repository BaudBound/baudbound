const LIST_PAGES = `
  query WikiPages {
    pages {
      list(orderBy: PATH) {
        id path locale title description isPublished isPrivate tags
      }
    }
  }
`;

const READ_PAGE = `
  query PageSource($id: Int!) {
    pages {
      single(id: $id) {
        id path locale title description isPublished isPrivate editor content tags { tag }
      }
    }
  }
`;

const CREATE_PAGE = `
  mutation CreatePage($content: String!, $description: String!, $editor: String!, $isPublished: Boolean!, $isPrivate: Boolean!, $locale: String!, $path: String!, $tags: [String]!, $title: String!) {
    pages {
      create(content: $content, description: $description, editor: $editor, isPublished: $isPublished, isPrivate: $isPrivate, locale: $locale, path: $path, tags: $tags, title: $title) {
        responseResult { succeeded errorCode slug message }
        page { id path }
      }
    }
  }
`;

const UPDATE_PAGE = `
  mutation UpdatePage($id: Int!, $content: String!, $description: String!, $editor: String!, $isPublished: Boolean!, $isPrivate: Boolean!, $locale: String!, $path: String!, $tags: [String]!, $title: String!) {
    pages {
      update(id: $id, content: $content, description: $description, editor: $editor, isPublished: $isPublished, isPrivate: $isPrivate, locale: $locale, path: $path, tags: $tags, title: $title) {
        responseResult { succeeded errorCode slug message }
        page { id path }
      }
    }
  }
`;

const DELETE_PAGE = `
  mutation DeletePage($id: Int!) {
    pages {
      delete(id: $id) { responseResult { succeeded errorCode slug message } }
    }
  }
`;

const READ_NAVIGATION = `
  query WikiNavigation {
    navigation {
      config { mode }
      tree {
        locale
        items { id kind label icon targetType target visibilityMode visibilityGroups }
      }
    }
  }
`;

const UPDATE_NAVIGATION = `
  mutation UpdateNavigation($mode: NavigationMode!, $tree: [NavigationTreeInput]!) {
    navigation {
      updateTree(tree: $tree) {
        responseResult { succeeded errorCode slug message }
      }
      updateConfig(mode: $mode) {
        responseResult { succeeded errorCode slug message }
      }
    }
  }
`;

export class WikiJsClient {
  constructor({ baseUrl, token, fetchImpl = fetch }) {
    const url = new URL(baseUrl);
    if (url.protocol !== "https:") {
      throw new Error("WIKI_URL must use HTTPS");
    }
    if (!token?.trim()) {
      throw new Error("WIKI_API_TOKEN is required");
    }
    this.endpoint = new URL("graphql", ensureTrailingSlash(url)).toString();
    this.fetchImpl = fetchImpl;
    this.token = token;
  }

  async listPages() {
    const data = await this.request(LIST_PAGES, {});
    return data.pages.list;
  }

  async readPage(id) {
    const data = await this.request(READ_PAGE, { id });
    if (!data.pages.single) throw new Error(`Wiki.js page ${id} no longer exists`);
    return {
      ...data.pages.single,
      tags: data.pages.single.tags.map((tag) => tag.tag),
    };
  }

  async createPage(page) {
    const data = await this.request(CREATE_PAGE, page);
    assertMutation(data.pages.create.responseResult, `create ${page.path}`);
  }

  async updatePage(id, page) {
    const data = await this.request(UPDATE_PAGE, { id, ...page });
    assertMutation(data.pages.update.responseResult, `update ${page.path}`);
  }

  async deletePage(page) {
    const data = await this.request(DELETE_PAGE, { id: page.id });
    assertMutation(data.pages.delete.responseResult, `delete ${page.path}`);
  }

  async readNavigation() {
    const data = await this.request(READ_NAVIGATION, {});
    return { mode: data.navigation.config.mode, tree: data.navigation.tree };
  }

  async updateNavigation(navigation) {
    const data = await this.request(UPDATE_NAVIGATION, navigation);
    assertMutation(data.navigation.updateTree.responseResult, "update navigation tree");
    assertMutation(data.navigation.updateConfig.responseResult, "update navigation mode");
  }

  async request(query, variables) {
    let lastError;
    for (let attempt = 1; attempt <= 3; attempt += 1) {
      try {
        const response = await this.fetchImpl(this.endpoint, {
          body: JSON.stringify({ query, variables }),
          headers: {
            authorization: `Bearer ${this.token}`,
            "content-type": "application/json",
          },
          method: "POST",
          signal: AbortSignal.timeout(15_000),
        });
        if (!response.ok) {
          const message = `Wiki.js GraphQL returned HTTP ${response.status}`;
          if (attempt < 3 && (response.status === 429 || response.status >= 500)) {
            lastError = new Error(message);
            await delay(attempt * 500);
            continue;
          }
          throw new Error(message);
        }
        const payload = await response.json();
        if (payload.errors?.length) {
          throw new Error(payload.errors.map((error) => error.message).join("; "));
        }
        if (!payload.data) throw new Error("Wiki.js GraphQL returned no data");
        return payload.data;
      } catch (error) {
        lastError = error;
        if (attempt === 3 || !isRetryableNetworkError(error)) throw error;
        await delay(attempt * 500);
      }
    }
    throw lastError;
  }
}

function assertMutation(result, operation) {
  if (!result?.succeeded) {
    throw new Error(
      `Wiki.js failed to ${operation}: ${result?.message || result?.slug || "unknown error"}`,
    );
  }
}

function ensureTrailingSlash(url) {
  return new URL(url.pathname.endsWith("/") ? url.pathname : `${url.pathname}/`, url);
}

function isRetryableNetworkError(error) {
  return error instanceof TypeError || error?.name === "TimeoutError";
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
