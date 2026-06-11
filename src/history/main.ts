import { mount } from "svelte";
import "../lib/theme.css";
import History from "./History.svelte";

const target = document.getElementById("app");
if (target) {
  mount(History, { target });
}
