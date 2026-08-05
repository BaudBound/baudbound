import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";

describe("private runner inputs", () => {
  it("disables browser form-value storage on text inputs", () => {
    expect(renderToStaticMarkup(<Input />)).toContain('autoComplete="off"');
  });

  it("disables browser form-value storage on text areas", () => {
    expect(renderToStaticMarkup(<Textarea />)).toContain('autoComplete="off"');
  });
});
