<script setup lang="ts">
import pulseLogo from "./assets/pulse.svg";
</script>

<template>
  <div id="pulse-app">
    <nav class="top-nav">
      <div class="nav-brand">
        <img :src="pulseLogo" alt="Pulse" class="nav-logo" />
        Pulse Gateway
      </div>
      <div class="nav-links">
        <router-link to="/">Dashboard</router-link>
        <router-link to="/settings">Settings</router-link>
      </div>
    </nav>
    <main class="content">
      <router-view />
    </main>
  </div>
</template>

<style>
:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;
  color: #0f0f0f;
  background-color: #f6f6f6;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  padding: 0;
}

.top-nav {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1.5rem;
  background: #1a1a2e;
  color: #fff;
}

.nav-brand {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 700;
  font-size: 1.1rem;
}

.nav-logo {
  width: 24px;
  height: 24px;
}

.nav-links {
  display: flex;
  gap: 1.5rem;
}

.nav-links a {
  color: #aaa;
  text-decoration: none;
  font-weight: 500;
  font-size: 0.95rem;
}

.nav-links a:hover,
.nav-links a.router-link-active {
  color: #fff;
}

.content {
  max-width: 900px;
  margin: 0 auto;
  padding: 1rem;
}

input,
button {
  border-radius: 6px;
  border: 1px solid #ccc;
  padding: 0.5rem 0.8rem;
  font-size: 0.95rem;
  font-family: inherit;
}

button {
  cursor: pointer;
  background: #396cd8;
  color: #fff;
  border-color: transparent;
  font-weight: 500;
}

button:hover {
  background: #2a5ab8;
}

button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

@media (prefers-color-scheme: dark) {
  :root {
    color: #f6f6f6;
    background-color: #0f0f0f;
  }

  input {
    background: #1a1a1a;
    border-color: #444;
    color: #f6f6f6;
  }
}
</style>
