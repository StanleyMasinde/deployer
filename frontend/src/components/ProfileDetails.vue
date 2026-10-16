<script setup lang="ts">
defineProps<{ user?: { id: number; name: string; email: string } }>()
</script>
<template>
  <section class="dashboard-panel profile-panel">
    <h2>Account details</h2>
    <dl class="detail-values">
      <div>
        <dt>Name</dt>
        <dd>{{ user?.name ?? 'Unavailable' }}</dd>
      </div>
      <div>
        <dt>Email</dt>
        <dd>{{ user?.email ?? 'Unavailable' }}</dd>
      </div>
      <div>
        <dt>Role</dt>
        <dd>Instance owner</dd>
      </div>
    </dl>
    <RouterLink class="button" to="/console/personal">Personal settings</RouterLink>
  </section>
</template>
