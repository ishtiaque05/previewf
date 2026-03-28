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
                var stored = localStorage.getItem(THEME_KEY);
                if (!stored) {
                    applyTheme(e.matches ? 'dark' : 'light');
                }
            });
        }
    }

    /* ----------------------------------------------------------------------
       2. Navigation Sidebar (File Tree)
       ---------------------------------------------------------------------- */

    var NAV_KEY = 'previewf-nav';
    var NAV_OPEN_DIRS_KEY = 'previewf-nav-open';

    function initNavSidebar() {
        var sidebar = document.getElementById('nav-sidebar');
        var toggle = document.getElementById('nav-toggle');
        var treeEl = document.getElementById('nav-tree');
        if (!sidebar || !toggle) return;

        // Restore collapsed state
        var navState = localStorage.getItem(NAV_KEY);
        if (navState === 'collapsed') {
            sidebar.classList.add('collapsed');
        }

        toggle.addEventListener('click', function () {
            sidebar.classList.toggle('collapsed');
            localStorage.setItem(NAV_KEY, sidebar.classList.contains('collapsed') ? 'collapsed' : 'open');
        });

        // Fetch and render tree
        if (treeEl) {
            fetch('/api/tree')
                .then(function (r) { return r.json(); })
                .then(function (tree) {
                    renderTree(treeEl, tree, 0);
                    highlightCurrentFile();
                })
                .catch(function () {
                    // silently fail — tree is nice to have
                });
        }
    }

    function getOpenDirs() {
        try {
            var stored = localStorage.getItem(NAV_OPEN_DIRS_KEY);
            return stored ? JSON.parse(stored) : {};
        } catch (e) {
            return {};
        }
    }

    function saveOpenDirs(dirs) {
        localStorage.setItem(NAV_OPEN_DIRS_KEY, JSON.stringify(dirs));
    }

    function renderTree(container, nodes, depth) {
        var openDirs = getOpenDirs();

        for (var i = 0; i < nodes.length; i++) {
            var node = nodes[i];

            if (node.type === 'dir') {
                renderDirNode(container, node, depth, openDirs);
            } else {
                renderFileNode(container, node, depth);
            }
        }
    }

    function renderDirNode(container, node, depth, openDirs) {
        var isOpen = openDirs[node.path] === true;

        // Directory row
        var row = document.createElement('div');
        row.className = 'tree-item tree-depth-' + Math.min(depth, 5);
        row.setAttribute('data-path', node.path);

        var arrow = document.createElement('span');
        arrow.className = 'tree-item-icon dir-arrow' + (isOpen ? ' open' : '');
        arrow.textContent = '\u25B6'; // right triangle
        row.appendChild(arrow);

        var icon = document.createElement('span');
        icon.className = 'tree-item-icon tree-icon-dir';
        icon.textContent = '\uD83D\uDCC1'; // folder emoji
        row.appendChild(icon);

        var name = document.createElement('span');
        name.className = 'tree-item-name';
        name.textContent = node.name;
        row.appendChild(name);

        container.appendChild(row);

        // Children container
        var childrenEl = document.createElement('div');
        childrenEl.className = 'tree-children' + (isOpen ? ' open' : '');
        container.appendChild(childrenEl);

        if (node.children && node.children.length > 0) {
            renderTree(childrenEl, node.children, depth + 1);
        }

        // Toggle on click
        row.addEventListener('click', function () {
            var dirs = getOpenDirs();
            var nowOpen = !childrenEl.classList.contains('open');
            childrenEl.classList.toggle('open');
            arrow.classList.toggle('open');
            dirs[node.path] = nowOpen;
            saveOpenDirs(dirs);
        });
    }

    function renderFileNode(container, node, depth) {
        var link = document.createElement('a');
        link.className = 'tree-item tree-depth-' + Math.min(depth, 5);

        // Determine href based on type
        if (node.type === 'md' || node.type === 'json') {
            link.href = '/view/' + node.path;
        } else if (node.type === 'html') {
            link.href = '/raw/' + node.path;
        }

        var icon = document.createElement('span');
        icon.className = 'tree-item-icon';
        if (node.type === 'md') {
            icon.className += ' tree-icon-md';
            icon.textContent = '\u25C6'; // diamond
        } else if (node.type === 'json') {
            icon.className += ' tree-icon-json';
            icon.textContent = '{}';
        } else {
            icon.className += ' tree-icon-html';
            icon.textContent = '\u25C7'; // open diamond
        }
        link.appendChild(icon);

        var name = document.createElement('span');
        name.className = 'tree-item-name';
        name.textContent = node.name;
        link.appendChild(name);

        container.appendChild(link);
    }

    function highlightCurrentFile() {
        var path = window.location.pathname;
        // Extract the file/dir path from the URL
        var match = path.match(/^\/(view|raw|browse)\/(.+)$/);
        if (!match) return;

        var filePath = match[2];

        // Highlight matching tree item
        var items = document.querySelectorAll('.tree-item[data-path]');
        for (var i = 0; i < items.length; i++) {
            if (items[i].getAttribute('data-path') === filePath) {
                items[i].classList.add('active');
            }
        }

        // Also highlight file links
        var links = document.querySelectorAll('a.tree-item');
        for (var j = 0; j < links.length; j++) {
            var href = links[j].getAttribute('href');
            if (href === path) {
                links[j].classList.add('active');
            }
        }

        // Auto-expand parent directories
        var parts = filePath.split('/');
        var openDirs = getOpenDirs();
        var changed = false;
        var accumulated = '';
        for (var k = 0; k < parts.length - 1; k++) {
            accumulated = accumulated ? accumulated + '/' + parts[k] : parts[k];
            if (!openDirs[accumulated]) {
                openDirs[accumulated] = true;
                changed = true;
            }
        }
        if (changed) {
            saveOpenDirs(openDirs);
            // Re-open the expanded dirs in DOM
            var allArrows = document.querySelectorAll('.tree-item[data-path]');
            for (var m = 0; m < allArrows.length; m++) {
                var p = allArrows[m].getAttribute('data-path');
                if (openDirs[p]) {
                    var arrowEl = allArrows[m].querySelector('.dir-arrow');
                    var sibling = allArrows[m].nextElementSibling;
                    if (arrowEl && !arrowEl.classList.contains('open')) {
                        arrowEl.classList.add('open');
                    }
                    if (sibling && sibling.classList.contains('tree-children') && !sibling.classList.contains('open')) {
                        sibling.classList.add('open');
                    }
                }
            }
        }
    }

    /* ----------------------------------------------------------------------
       3. Flag Sidebar Population
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
       4. Flag Creation Toolbar
       ---------------------------------------------------------------------- */

    var toolbar = null;
    var currentFilepath = '';

    function initFlagToolbar() {
        var documentEl = document.getElementById('document');
        if (!documentEl) return;

        // Extract filepath from breadcrumb
        var breadcrumbCurrent = document.querySelector('.breadcrumb-current');
        var breadcrumbLinks = document.querySelectorAll('.breadcrumb-link');
        if (breadcrumbCurrent) {
            // Build path from breadcrumb links + current
            var parts = [];
            for (var i = 1; i < breadcrumbLinks.length; i++) { // skip 'root'
                parts.push(breadcrumbLinks[i].textContent.trim());
            }
            parts.push(breadcrumbCurrent.textContent.trim());
            currentFilepath = parts.join('/');
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
        var top = y + scrollY + 10;

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

        var url = '/flag/' + currentFilepath.split('/').map(encodeURIComponent).join('/');
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
       5. WebSocket Live Reload
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
                var textNode = statusDot.nextSibling;
                if (textNode && textNode.nodeType === Node.TEXT_NODE) {
                    textNode.textContent = '\n            connected\n        ';
                } else {
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
        initNavSidebar();
        initFlagSidebar();
        initFlagToolbar();
        initWebSocket();
    });

})();
