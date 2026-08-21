document.addEventListener('DOMContentLoaded', () => {
  const toggle = document.getElementById('bitchModeToggle');
  const statusText = document.getElementById('bitchStatus');
  if (!toggle || !statusText) return;
  toggle.addEventListener('change', (e) => {
    if (e.target.checked) {
      document.body.classList.add('bitch-mode');
      statusText.textContent = 'ON 🔥';
      statusText.className = 'status-on';
    } else {
      document.body.classList.remove('bitch-mode');
      statusText.textContent = 'OFF';
      statusText.className = 'status-off';
    }
  });
});
