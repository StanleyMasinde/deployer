<script setup lang="ts">
definePage({
  name: "home",
  meta: { ssg: true, title: "Deployer" },
});
</script>

<template>
  <main id="main" class="welcome">
    <p class="eyebrow">SELF-HOSTED / STATIC DEPLOYMENTS</p>
    <h1>A small home<br />for the apps you ship.</h1>
    <div class="intro-grid">
      <p class="lede">
        Connect a repository, review the workflow, and put your app on your own VPS. GitHub builds
        it. Deployer brings it home.
      </p>
      <RouterLink class="button primary" to="/console"
        >Open your console <span aria-hidden="true">→</span></RouterLink
      >
    </div>
    <ol class="pipeline" aria-label="Deployment flow">
      <li>
        <span class="step-number">01</span>
        <h2>Connect</h2>
        <p>Your GitHub App grants access to the repositories you choose.</p>
      </li>
      <li>
        <span class="step-number">02</span>
        <h2>Review & build</h2>
        <p>Merge the workflow PR. Builds run on GitHub Actions.</p>
      </li>
      <li>
        <span class="step-number">03</span>
        <h2>Publish</h2>
        <p>A signed webhook queues the artifact for your worker. Nginx serves the result.</p>
      </li>
    </ol>
    <aside class="architecture">
      <span class="eyebrow">ON YOUR VPS</span>
      <p>Two native processes. Static files. No build containers.</p>
    </aside>
  </main>
</template>
