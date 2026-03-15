import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/600.css";
import "@fontsource/jetbrains-mono/700.css";

import { App } from "./App";
import "./styles.css";

const container = document.getElementById("root");

if (container === null) {
  throw new Error("Missing #root container for web console bootstrap.");
}

const rootElement = document.documentElement;
rootElement.dataset.theme = "dark";
rootElement.classList.add("dark");

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>
);
