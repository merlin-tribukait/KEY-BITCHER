const QUOTES = {
  bitchModeOn: '"BITCH MODE ACTIVE: 10x increased sarcasm, aggressive error messages unlocked."',
  bitchModeOff: '"If it\'s broken, expect her to complain. If it works, stop touching it."'
};

const STATUS_TEXT = {
  on: 'ON 🔥',
  off: 'OFF'
};

document.addEventListener('DOMContentLoaded', () => {
  const toggle = document.getElementById('bitchModeToggle');
  const statusText = document.getElementById('bitchStatus');
  const quote = document.getElementById('dynamicQuote');
  const form = document.getElementById('grievanceForm');
  const modal = document.getElementById('grievanceResponse');

  toggle.addEventListener('change', (e) => {
    const isActive = e.target.checked;
    document.body.classList.toggle('bitch-mode', isActive);
    statusText.textContent = isActive ? STATUS_TEXT.on : STATUS_TEXT.off;
    statusText.className = isActive ? 'status-on' : 'status-off';
    quote.textContent = isActive ? QUOTES.bitchModeOn : QUOTES.bitchModeOff;
  });

  form.addEventListener('submit', (e) => {
    e.preventDefault();
    modal.classList.remove('hidden');
  });
});

function copyCode(id) {
  const text = document.getElementById(id).textContent;
  navigator.clipboard.writeText(text).catch(err => {
    console.error('Failed to copy:', err);
    alert('Failed to copy. Try again?');
  });
}

function closeModal() {
  document.getElementById('grievanceResponse').classList.add('hidden');
}
