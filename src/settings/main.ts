import { mount } from "svelte";
import "../lib/theme.css";
import Settings from "./Settings.svelte";

const target = document.getElementById("app");
if (target) {
  mount(Settings, { target });
}
