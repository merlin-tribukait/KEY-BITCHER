(function () {
  'use strict';

  document.documentElement.className += ' js';

  var BITCH_JOKES = [
    'You don\u2019t have a bug. You have a feature that committed its own API key.',
    'Error message #42: \u201CYour .env has opinions. Your .env is wrong.\u201D',
    'I\u2019ve seen cleaner code in a gutter. At least the gutter is honest about what it collects.',
    'The vault isn\u2019t locked for security. It\u2019s locked to keep your commits out of it.',
    'Somewhere, a secret is exposed. It\u2019s yours, and the Bitch is delighted.',
    'Rotating keys is like admitting you trusted the wrong person. That person was future-you.',
    '\u201CIt works on my machine\u201D \u2014 yes, she heard that, and she logged it. Publicly.',
    'Every keystroke you make, she watches. Every key you commit, she publishes.',
    'The docs are angry because the code is lazy.',
    'If you can\u2019t handle the Bitch at her worst, you don\u2019t deserve her at her least-broken.',
    'She doesn\u2019t have a debug mode. She has a judgment mode. It\u2019s always on.',
    'Hot take: your secrets are only as safe as the intern who reads the log files.',
    'A key a day keeps the incident-response team away. That is not true. Rotate anyway.',
    'The Bitch once rotated a key mid-deploy. Twice. Nobody rotates keys anymore.'
  ];

  function fillBitchQuotes() {
    var els = document.querySelectorAll('.bitch-quote p');
    if (!els.length) return;
    var pool = BITCH_JOKES.slice();
    for (var i = pool.length - 1; i > 0; i--) {
      var j = Math.floor(Math.random() * (i + 1));
      var t = pool[i]; pool[i] = pool[j]; pool[j] = t;
    }
    els.forEach(function (el, idx) {
      el.textContent = pool[idx % pool.length];
    });
  }

  document.addEventListener('DOMContentLoaded', function () {
    var reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    fillBitchQuotes();

    /* Off-canvas mobile menu */
    var menuBtn = document.querySelector('.mobile-menu-btn');
    if (menuBtn && document.querySelector('.sidebar')) {
      var backdrop = document.createElement('div');
      backdrop.className = 'sidebar-backdrop';
      document.body.appendChild(backdrop);

      function setMenu(open) {
        document.body.classList.toggle('menu-open', open);
        menuBtn.classList.toggle('active', open);
        menuBtn.setAttribute('aria-expanded', open ? 'true' : 'false');
      }
      menuBtn.addEventListener('click', function () {
        setMenu(!document.body.classList.contains('menu-open'));
      });
      backdrop.addEventListener('click', function () { setMenu(false); });
      document.addEventListener('keydown', function (e) {
        if (e.key === 'Escape') setMenu(false);
      });
      document.querySelectorAll('.nav-link').forEach(function (a) {
        a.addEventListener('click', function () { setMenu(false); });
      });
      var mq = window.matchMedia('(min-width: 901px)');
      if (mq.addEventListener) {
        mq.addEventListener('change', function (e) { if (e.matches) setMenu(false); });
      }
    }

    /* Scroll-in reveal for sections and cards */
    var revealEls = document.querySelectorAll('.reveal, .docs-content h2, .docs-content h3, ' +
      '.docs-content p, .docs-content ul, .docs-content ol, .docs-content table, ' +
      '.docs-content pre, .docs-content blockquote');
    if (reduced || !('IntersectionObserver' in window)) {
      revealEls.forEach(function (el) { el.classList.add('revealed'); });
    } else {
      var io = new IntersectionObserver(function (entries) {
        entries.forEach(function (entry) {
          if (entry.isIntersecting) {
            entry.target.classList.add('revealed');
            io.unobserve(entry.target);
          }
        });
      }, { threshold: 0.08, rootMargin: '0px 0px -6% 0px' });
      revealEls.forEach(function (el) { io.observe(el); });
    }

    /* Back-to-top button */
    var topBtn = document.createElement('button');
    topBtn.id = 'backToTop';
    topBtn.setAttribute('aria-label', 'Back to top');
    topBtn.textContent = '↑';
    document.body.appendChild(topBtn);
    var showing = false;
    var onScroll = function () {
      var past = window.scrollY > 320;
      if (past !== showing) {
        showing = past;
        topBtn.classList.toggle('visible', past);
      }
    };
    onScroll();
    window.addEventListener('scroll', onScroll, { passive: true });
    topBtn.addEventListener('click', function () {
      window.scrollTo({ top: 0, behavior: reduced ? 'auto' : 'smooth' });
    });

    /* Gentle parallax on hero badges while hovering the page */
    if (!reduced && window.matchMedia('(pointer: fine)').matches) {
      var badges = document.querySelectorAll('.hero-badge, .goblin-teaser-card img');
      badges.forEach(function (badge) {
        badge.addEventListener('mousemove', function (e) {
          var r = badge.getBoundingClientRect();
          var dx = (e.clientX - (r.left + r.width / 2)) / r.width;
          var dy = (e.clientY - (r.top + r.height / 2)) / r.height;
          badge.style.transform = 'translate(' + (dx * 8) + 'px,' + (dy * 8) + 'px)';
        });
        badge.addEventListener('mouseleave', function () {
          badge.style.transform = '';
        });
      });
    }
  });
})();
