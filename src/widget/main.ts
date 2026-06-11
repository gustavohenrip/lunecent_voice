import { mount } from "svelte";
import "../lib/theme.css";
import Widget from "./Widget.svelte";

const target = document.getElementById("app");
if (target) {
  mount(Widget, { target });
}
