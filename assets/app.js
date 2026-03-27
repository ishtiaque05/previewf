/* ==========================================================================
   previewf — Client-side JavaScript
   SECURITY: All DOM manipulation uses safe methods (createElement, textContent,
   appendChild). No innerHTML usage anywhere.
   ========================================================================== */

(function () {
    'use strict';

    /* ----------------------------------------------------------------------
       1. Theme Toggle
       ---------------------------------------------------------------------- */

    var THEME_KEY = 'previewf-theme';

    function getPreferredTheme() {
        var stored = localStorage.getItem(THEME_KEY);
        if (stored === 'light' || stored === 'dark') {
            return stored;
        }
        if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
            return 'dark';
        }
        return 'light';
    }

    function applyTheme(theme) {
        document.documentElement.setAttribute('data-theme', theme);
    }

    function persistTheme(theme) {
        applyTheme(theme);
        localStorage.setItem(THEME_KEY, theme);
    }

    function initTheme() {
        var theme = getPreferredTheme();
        applyTheme(theme);

        var toggle = document.getElementById('theme-toggle');
        if (toggle) {
            toggle.addEventListener('click', function () {
                var current = document.documentElement.getAttribute('data-theme');
                var next = current === 'dark' ? 'light' : 'dark';
                persistTheme(next);
            });
        }

        // Listen for OS-level theme changes
        if (window.matchMedia) {
            window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function (e) {
                // Only follow OS preference if user hasn't manually set a theme
                var stored = localStorage.getItem(THEME_KEY);
                if (!stored) {
                    applyTheme(e.matches ? 'dark' : 'light');
                }
            });
        }
    }

    /* ----------------------------------------------------------------------
       2. Flag Sidebar Population
       ---------------------------------------------------------------------- */

    function initFlagSidebar() {
        var flagList = document.getElementById('flag-list');
        var flagCountEl = document.getElementById('flag-count');
        if (!flagList) return;

        var flags = document.querySelectorAll('.flag[data-flag-id]');

        // Update flag count badge
        if (flagCountEl) {
            flagCountEl.textContent = String(flags.length);
        }

        if (flags.length === 0) {
            var emptyMsg = document.createElement('p');
            emptyMsg.className = 'flag-list-empty';
            emptyMsg.textContent = 'No flags in this document.';
            emptyMsg.style.fontSize = '0.82rem';
            emptyMsg.style.color = 'var(--text-muted)';
            emptyMsg.style.fontFamily = "'DM Sans', system-ui, sans-serif";
            flagList.appendChild(emptyMsg);
            return;
        }

        for (var i = 0; i < flags.length; i++) {
            var flagEl = flags[i];
            var flagId = flagEl.getAttribute('data-flag-id');
            var flagCommentEl = flagEl.querySelector('.flag-comment');
            var flagComment = flagCommentEl ? flagCommentEl.textContent : '';

            var item = createFlagItem(flagId, flagComment, flagEl);
            flagList.appendChild(item);
        }

        // Click flag in document -> highlight in sidebar
        for (var j = 0; j < flags.length; j++) {
            (function (flagElement) {
                flagElement.addEventListener('click', function () {
                    var id = flagElement.getAttribute('data-flag-id');
                    var sidebarItem = document.querySelector('.flag-item[data-flag-id="' + id + '"]');
                    if (sidebarItem) {
                        sidebarItem.scrollIntoView({ behavior: 'smooth', block: 'center' });
                        sidebarItem.classList.add('flag-highlight');
                        setTimeout(function () {
                            sidebarItem.classList.remove('flag-highlight');
                        }, 2000);
                    }
                });
            })(flags[j]);
        }
    }

    function createFlagItem(flagId, comment, flagElement) {
        var item = document.createElement('div');
        item.className = 'flag-item';
        item.setAttribute('data-flag-id', flagId);

        var header = document.createElement('div');
        header.className = 'flag-item-header';

        var idLabel = document.createElement('span');
        idLabel.className = 'flag-item-id';
        idLabel.textContent = 'Flag #' + flagId;

        header.appendChild(idLabel);
        item.appendChild(header);

        if (comment) {
            var commentEl = document.createElement('div');
            commentEl.className = 'flag-item-comment';
            commentEl.textContent = comment;
            item.appendChild(commentEl);
        }

        // Click sidebar item -> scroll to document flag
        item.addEventListener('click', function () {
            if (flagElement) {
                flagElement.scrollIntoView({ behavior: 'smooth', block: 'center' });
                flagElement.classList.add('flag-highlight');
                setTimeout(function () {
                    flagElement.classList.remove('flag-highlight');
                }, 2000);
            }
        });

        return item;
    }

    /* ----------------------------------------------------------------------
       3. Flag Creation Toolbar
       ---------------------------------------------------------------------- */

    var toolbar = null;
    var currentFilepath = '';

    function initFlagToolbar() {
        var documentEl = document.getElementById('document');
        if (!documentEl) return;

        // Extract filepath from the top bar
        var filePathEl = document.querySelector('.file-path');
        if (filePathEl) {
            currentFilepath = filePathEl.textContent.trim();
        }

        // Create the toolbar element
        toolbar = document.createElement('div');
        toolbar.className = 'flag-toolbar';

        var label = document.createElement('span');
        label.className = 'flag-toolbar-label';
        label.textContent = 'Add flag';
        toolbar.appendChild(label);

        var input = document.createElement('input');
        input.className = 'flag-toolbar-input';
        input.type = 'text';
        input.placeholder = 'Comment...';
        input.setAttribute('aria-label', 'Flag comment');
        toolbar.appendChild(input);

        var actions = document.createElement('div');
        actions.className = 'flag-toolbar-actions';

        var cancelBtn = document.createElement('button');
        cancelBtn.className = 'flag-toolbar-btn flag-toolbar-btn-cancel';
        cancelBtn.type = 'button';
        cancelBtn.textContent = 'Cancel';
        actions.appendChild(cancelBtn);

        var submitBtn = document.createElement('button');
        submitBtn.className = 'flag-toolbar-btn flag-toolbar-btn-submit';
        submitBtn.type = 'button';
        submitBtn.textContent = 'Flag';
        actions.appendChild(submitBtn);

        toolbar.appendChild(actions);
        document.body.appendChild(toolbar);

        // State for current selection
        var selectedText = '';

        // Show toolbar on text selection in document
        documentEl.addEventListener('mouseup', function (e) {
            var selection = window.getSelection();
            var text = selection ? selection.toString().trim() : '';

            if (text.length > 0) {
                selectedText = text;
                positionToolbar(e.clientX, e.clientY);
                toolbar.classList.add('active');
                input.value = '';
                // Defer focus so toolbar appears first
                setTimeout(function () {
                    input.focus();
                }, 50);
            }
        });

        // Cancel
        cancelBtn.addEventListener('click', function () {
            hideToolbar();
        });

        // Submit
        submitBtn.addEventListener('click', function () {
            submitFlag(input.value.trim(), selectedText);
        });

        // Submit on Enter key
        input.addEventListener('keydown', function (e) {
            if (e.key === 'Enter') {
                e.preventDefault();
                submitFlag(input.value.trim(), selectedText);
            }
        });

        // Hide on Escape
        document.addEventListener('keydown', function (e) {
            if (e.key === 'Escape') {
                hideToolbar();
            }
        });

        // Hide on click outside
        document.addEventListener('mousedown', function (e) {
            if (toolbar && toolbar.classList.contains('active') && !toolbar.contains(e.target)) {
                hideToolbar();
            }
        });
    }

    function positionToolbar(x, y) {
        if (!toolbar) return;

        var scrollX = window.scrollX || window.pageXOffset;
        var scrollY = window.scrollY || window.pageYOffset;

        var left = x + scrollX;
        var top = y + scrollY + 10; // Slightly below the cursor

        // Keep toolbar within viewport horizontally
        var toolbarWidth = 280;
        if (left + toolbarWidth > document.documentElement.scrollWidth) {
            left = document.documentElement.scrollWidth - toolbarWidth - 16;
        }
        if (left < 16) {
            left = 16;
        }

        toolbar.style.left = left + 'px';
        toolbar.style.top = top + 'px';
    }

    function hideToolbar() {
        if (toolbar) {
            toolbar.classList.remove('active');
        }
    }

    function submitFlag(comment, selectedText) {
        if (!comment || !selectedText || !currentFilepath) {
            hideToolbar();
            return;
        }

        var url = '/flag/' + encodeURIComponent(currentFilepath);
        var body = JSON.stringify({
            comment: comment,
            selected_text: selectedText
        });

        fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: body
        })
        .then(function (response) {
            if (response.ok) {
                // Reload to show the new flag
                window.location.reload();
            } else {
                response.text().then(function (text) {
                    var errorMsg = 'Failed to create flag';
                    if (text) {
                        errorMsg += ': ' + text;
                    }
                    showStatusMessage(errorMsg, true);
                });
            }
        })
        .catch(function (err) {
            showStatusMessage('Network error creating flag: ' + err.message, true);
        });

        hideToolbar();
    }

    function showStatusMessage(message, isError) {
        var statusBar = document.querySelector('.status-bar');
        if (!statusBar) return;

        var msg = document.createElement('span');
        msg.textContent = ' \u00B7 ' + message;
        msg.style.color = isError ? '#EF4444' : 'var(--link)';
        msg.style.fontWeight = '600';
        statusBar.appendChild(msg);

        setTimeout(function () {
            if (msg.parentNode) {
                msg.parentNode.removeChild(msg);
            }
        }, 4000);
    }

    /* ----------------------------------------------------------------------
       4. WebSocket Live Reload
       ---------------------------------------------------------------------- */

    var ws = null;
    var reconnectTimer = null;
    var RECONNECT_DELAY = 2000;

    function initWebSocket() {
        var statusConnection = document.getElementById('status-connection');
        var statusDot = statusConnection ? statusConnection.querySelector('.status-dot') : null;

        function setConnected(connected) {
            if (!statusConnection || !statusDot) return;

            if (connected) {
                statusDot.classList.remove('disconnected');
                // Update the text node — find text node after the dot
                var textNode = statusDot.nextSibling;
                if (textNode && textNode.nodeType === Node.TEXT_NODE) {
                    textNode.textContent = '\n            connected\n        ';
                } else {
                    // Replace text content safely: rebuild children
                    while (statusConnection.lastChild && statusConnection.lastChild !== statusDot) {
                        statusConnection.removeChild(statusConnection.lastChild);
                    }
                    var txt = document.createTextNode('connected');
                    statusConnection.appendChild(txt);
                }
            } else {
                statusDot.classList.add('disconnected');
                var textNode2 = statusDot.nextSibling;
                if (textNode2 && textNode2.nodeType === Node.TEXT_NODE) {
                    textNode2.textContent = '\n            reconnecting\u2026\n        ';
                } else {
                    while (statusConnection.lastChild && statusConnection.lastChild !== statusDot) {
                        statusConnection.removeChild(statusConnection.lastChild);
                    }
                    var txt2 = document.createTextNode('reconnecting\u2026');
                    statusConnection.appendChild(txt2);
                }
            }
        }

        function connect() {
            // Build the WebSocket URL from the current page location
            var protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            var wsUrl = protocol + '//' + window.location.host + '/ws';

            try {
                ws = new WebSocket(wsUrl);
            } catch (e) {
                setConnected(false);
                scheduleReconnect();
                return;
            }

            ws.onopen = function () {
                setConnected(true);
                if (reconnectTimer) {
                    clearTimeout(reconnectTimer);
                    reconnectTimer = null;
                }
            };

            ws.onmessage = function (event) {
                var data = typeof event.data === 'string' ? event.data.trim() : '';
                if (data === 'reload') {
                    window.location.reload();
                }
            };

            ws.onclose = function () {
                setConnected(false);
                ws = null;
                scheduleReconnect();
            };

            ws.onerror = function () {
                setConnected(false);
                if (ws) {
                    ws.close();
                }
            };
        }

        function scheduleReconnect() {
            if (reconnectTimer) return;
            reconnectTimer = setTimeout(function () {
                reconnectTimer = null;
                connect();
            }, RECONNECT_DELAY);
        }

        connect();
    }

    /* ----------------------------------------------------------------------
       Initialize on DOMContentLoaded
       ---------------------------------------------------------------------- */

    document.addEventListener('DOMContentLoaded', function () {
        initTheme();
        initFlagSidebar();
        initFlagToolbar();
        initWebSocket();
    });

})();
