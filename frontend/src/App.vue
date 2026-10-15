<template>
  <a class="skip-link" href="#main">Skip to content</a>
  <header class="masthead">
    <RouterLink class="wordmark" to="/" aria-label="Deployer home">
      <span class="mark" aria-hidden="true">↥</span> deployer<span class="wordmark-dot">.</span>
    </RouterLink>
    <nav aria-label="Main navigation">
      <RouterLink to="/console">Console <span aria-hidden="true">↗</span></RouterLink>
    </nav>
  </header>
  <RouterView />
  <footer class="footer">
    <span>Your server. Your apps.</span><span>Static apps · GitHub Actions · Nginx</span>
  </footer>
</template>
