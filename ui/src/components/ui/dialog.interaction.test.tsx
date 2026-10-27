// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { Dialog, DialogOverlay, DialogPortal } from "./dialog";
afterEach(cleanup);
it("forwards the overlay DOM ref used by Radix presence under React 18", () => {
  const ref = createRef<HTMLDivElement>();
  const view = render(
    <Dialog open>
      <DialogPortal>
        <DialogOverlay ref={ref} data-testid="overlay" />
      </DialogPortal>
    </Dialog>,
  );
  expect(ref.current).toBe(screen.getByTestId("overlay"));
  view.unmount();
  expect(ref.current).toBeNull();
});
