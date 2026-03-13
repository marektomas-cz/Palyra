import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import "./styles.css";

const container = document.getElementById("root");

if (container === null) {
  throw new Error("Missing #root container for desktop UI bootstrap.");
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>
);
