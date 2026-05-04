/* ==========================================================================
   previewf — Client-side JavaScript
   SECURITY: All DOM manipulation uses safe methods (createElement, textContent,
   appendChild). No innerHTML usage anywhere.
   ========================================================================== */

(function () {
    'use strict';

    // Detect Docker context from current URL — used throughout for API paths
    var dockerMatch = window.location.pathname.match(/^\/docker\/([^/]+)/);
    var dockerContainer = dockerMatch ? dockerMatch[1] : null;
    var apiPrefix = dockerContainer
        ? '/docker/' + encodeURIComponent(dockerContainer)
        : '';

    /* ----------------------------------------------------------------------
       1. Theme Toggle
       ---------------------------------------------------------------------- */

    // Suppress WebSocket reload briefly after local flag mutations to avoid
    // a redundant full-page reload triggered by the file watcher.
    var suppressReload = false;
    function suppressReloadBriefly() {
        suppressReload = true;
        setTimeout(function () { suppressReload = false; }, 500);
    }

    var SIDEBAR_COLLAPSED_KEY = 'previewf-sidebar-collapsed';

    function initSidebarToggle() {
        var sidebar = document.getElementById('sidebar');
        var toggle = document.getElementById('sidebar-toggle');
        if (!sidebar || !toggle) return;

        if (localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === 'true') {
            sidebar.classList.add('collapsed');
            toggle.textContent = '\u203A';
        }

        toggle.addEventListener('click', function (e) {
            e.stopPropagation();
            var isCollapsed = sidebar.classList.toggle('collapsed');
            toggle.textContent = isCollapsed ? '\u203A' : '\u2039';
            localStorage.setItem(SIDEBAR_COLLAPSED_KEY, isCollapsed ? 'true' : 'false');
        });

        sidebar.addEventListener('click', function (e) {
            if (sidebar.classList.contains('collapsed') && e.target !== toggle) {
                sidebar.classList.remove('collapsed');
                toggle.textContent = '\u2039';
                localStorage.setItem(SIDEBAR_COLLAPSED_KEY, 'false');
            }
        });
    }

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

        // Fetch and render tree — use Docker endpoint when on a Docker page
        if (treeEl) {
            var treeUrl = dockerContainer
                ? '/docker/' + encodeURIComponent(dockerContainer) + '/api/tree'
                : '/api/tree';

            fetch(treeUrl)
                .then(function (r) {
                    if (!r.ok) throw new Error('Tree API returned ' + r.status);
                    return r.json();
                })
                .then(function (tree) {
                    renderTree(treeEl, tree, 0);
                    highlightCurrentFile();
                })
                .catch(function (err) {
                    console.warn('Failed to load file tree:', err);
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

    function encodeFilePath(p) {
        return p.split('/').map(encodeURIComponent).join('/');
    }

    function renderFileNode(container, node, depth) {
        var link = document.createElement('a');
        link.className = 'tree-item tree-depth-' + Math.min(depth, 5);

        // Determine href based on type — prefix with Docker path when applicable
        var encodedPath = encodeFilePath(node.path);
        var pathPrefix = dockerContainer
            ? '/docker/' + encodeURIComponent(dockerContainer)
            : '';
        if (node.type === 'md' || node.type === 'json') {
            link.href = pathPrefix + '/view/' + encodedPath;
        } else if (node.type === 'html') {
            link.href = pathPrefix + '/raw/' + encodedPath;
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
        // Extract the file/dir path from the URL (works for both local and Docker)
        var match = path.match(/^\/(view|raw|browse)\/(.+)$/)
            || path.match(/^\/docker\/[^/]+\/(view|raw|browse)\/(.+)$/);
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

    function refreshFlagSidebar() {
        if (!currentFilepath) return;

        var flagList = document.getElementById('flag-list');
        var flagCountEl = document.getElementById('flag-count');
        if (!flagList) return;

        var url = apiPrefix + '/flags/' + currentFilepath.split('/').map(encodeURIComponent).join('/');

        fetch(url)
            .then(function (r) {
                if (!r.ok) throw new Error('Flags API returned ' + r.status);
                return r.json();
            })
            .then(function (report) {
                // Clear existing items using safe DOM methods
                while (flagList.firstChild) {
                    flagList.removeChild(flagList.firstChild);
                }

                var flags = report.flags || [];

                // Update badge
                if (flagCountEl) {
                    flagCountEl.textContent = String(flags.length);
                }

                var sidebarBadge = document.getElementById('sidebar-badge');
                if (sidebarBadge) {
                    sidebarBadge.textContent = String(flags.length);
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
                    var item = createFlagItemFromData(flags[i]);
                    flagList.appendChild(item);
                }
            })
            .catch(function (err) {
                showStatusMessage('Failed to refresh flags. Reload the page.', true);
            });
    }

    function createFlagItemFromData(flag) {
        var item = document.createElement('div');
        item.className = 'flag-item';
        item.setAttribute('data-flag-id', flag.id);

        // Header
        var header = document.createElement('div');
        header.className = 'flag-item-header';
        var idLabel = document.createElement('span');
        idLabel.className = 'flag-item-id';
        idLabel.textContent = 'Flag #' + flag.id;
        header.appendChild(idLabel);

        var labelBadge = document.createElement('span');
        labelBadge.className = 'flag-label';
        labelBadge.setAttribute('data-label', flag.label.toLowerCase());
        labelBadge.textContent = flag.label;
        header.appendChild(labelBadge);

        item.appendChild(header);

        // Comment
        var commentEl = document.createElement('div');
        commentEl.className = 'flag-item-comment';
        commentEl.textContent = flag.comment;
        item.appendChild(commentEl);

        // Actions row
        var actions = document.createElement('div');
        actions.className = 'flag-item-actions';

        var editBtn = document.createElement('button');
        editBtn.className = 'flag-action-btn flag-action-btn-edit';
        editBtn.type = 'button';
        editBtn.textContent = 'Edit';
        actions.appendChild(editBtn);

        var deleteBtn = document.createElement('button');
        deleteBtn.className = 'flag-action-btn flag-action-btn-delete';
        deleteBtn.type = 'button';
        deleteBtn.textContent = 'Delete';
        actions.appendChild(deleteBtn);

        item.appendChild(actions);

        // Click item -> scroll to flag in document
        item.addEventListener('click', function (e) {
            if (e.target === editBtn || e.target === deleteBtn) return;
            var flagEl = document.querySelector('.flag[data-flag-id="' + flag.id + '"]');
            if (flagEl) {
                flagEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
                flagEl.classList.add('flag-highlight');
                setTimeout(function () {
                    flagEl.classList.remove('flag-highlight');
                }, 2000);
            }
        });

        // Delete handler
        deleteBtn.addEventListener('click', function () {
            var url = apiPrefix + '/flag/' + flag.id + '/' + currentFilepath.split('/').map(encodeURIComponent).join('/');
            fetch(url, { method: 'DELETE' })
                .then(function (r) {
                    if (!r.ok) throw new Error('Delete failed: ' + r.status);
                    suppressReloadBriefly();
                    refreshFlagSidebar();
                })
                .catch(function (err) {
                    showStatusMessage('Failed to delete flag: ' + err.message, true);
                });
        });

        // Edit handler
        editBtn.addEventListener('click', function () {
            enterEditMode(item, flag, commentEl, actions);
        });

        return item;
    }

    function enterEditMode(item, flag, commentEl, actionsEl) {
        // Hide comment and actions
        commentEl.style.display = 'none';
        actionsEl.style.display = 'none';

        // Create edit input
        var editContainer = document.createElement('div');
        editContainer.className = 'flag-edit-container';

        var editLabel = flag.label;
        var editLabelPicker = createLabelPicker(editLabel, function (pickedLabel) {
            editLabel = pickedLabel;
        });
        editContainer.appendChild(editLabelPicker);

        var input = document.createElement('input');
        input.className = 'flag-edit-input';
        input.type = 'text';
        input.value = flag.comment;
        editContainer.appendChild(input);

        var editActions = document.createElement('div');
        editActions.className = 'flag-edit-actions';

        var saveBtn = document.createElement('button');
        saveBtn.className = 'flag-action-btn flag-action-btn-save';
        saveBtn.type = 'button';
        saveBtn.textContent = 'Save';
        editActions.appendChild(saveBtn);

        var cancelBtn = document.createElement('button');
        cancelBtn.className = 'flag-action-btn flag-action-btn-cancel';
        cancelBtn.type = 'button';
        cancelBtn.textContent = 'Cancel';
        editActions.appendChild(cancelBtn);

        editContainer.appendChild(editActions);
        item.appendChild(editContainer);

        input.focus();
        input.select();

        function exitEditMode() {
            commentEl.style.display = '';
            actionsEl.style.display = '';
            if (editContainer.parentNode) {
                editContainer.parentNode.removeChild(editContainer);
            }
        }

        function saveEdit() {
            var newComment = input.value.trim();
            if (!newComment) {
                exitEditMode();
                return;
            }
            var url = apiPrefix + '/flag/' + flag.id + '/' + currentFilepath.split('/').map(encodeURIComponent).join('/');
            fetch(url, {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ comment: newComment, label: editLabel })
            })
            .then(function (r) {
                if (!r.ok) throw new Error('Update failed: ' + r.status);
                suppressReloadBriefly();
                refreshFlagSidebar();
            })
            .catch(function (err) {
                showStatusMessage('Failed to save: ' + err.message, true);
            });
        }

        saveBtn.addEventListener('click', saveEdit);
        cancelBtn.addEventListener('click', exitEditMode);
        input.addEventListener('keydown', function (e) {
            if (e.key === 'Enter') {
                e.preventDefault();
                saveEdit();
            }
            if (e.key === 'Escape') {
                exitEditMode();
            }
        });
    }

    function initFlagSidebar() {
        // Extract filepath from breadcrumb for API calls
        var breadcrumbCurrent = document.querySelector('.breadcrumb-current');
        var breadcrumbLinks = document.querySelectorAll('.breadcrumb-link');
        if (breadcrumbCurrent) {
            var parts = [];
            // Skip "root" (i=0) and on Docker pages also skip the container link (i=1)
            var startIdx = dockerContainer ? 2 : 1;
            for (var i = startIdx; i < breadcrumbLinks.length; i++) {
                parts.push(breadcrumbLinks[i].textContent.trim());
            }
            parts.push(breadcrumbCurrent.textContent.trim());
            currentFilepath = parts.join('/');
        }

        refreshFlagSidebar();
    }

    /* ----------------------------------------------------------------------
       4. Flag Creation Toolbar
       ---------------------------------------------------------------------- */

    var PREDEFINED_LABELS = ['Comment', 'Bug', 'Todo', 'Question', 'Note', 'Style'];

    function createLabelPicker(selectedLabel, onSelect) {
        var container = document.createElement('div');
        container.className = 'flag-label-picker';

        function renderPills() {
            while (container.firstChild) {
                container.removeChild(container.firstChild);
            }
            for (var i = 0; i < PREDEFINED_LABELS.length; i++) {
                (function (label) {
                    var pill = document.createElement('button');
                    pill.type = 'button';
                    pill.className = 'flag-label-pill';
                    pill.setAttribute('data-label', label.toLowerCase());
                    pill.textContent = label;
                    if (label === selectedLabel) {
                        pill.classList.add('selected');
                    }
                    pill.addEventListener('click', function (e) {
                        e.stopPropagation();
                        selectedLabel = label;
                        onSelect(label);
                        renderPills();
                    });
                    container.appendChild(pill);
                })(PREDEFINED_LABELS[i]);
            }
            var customBtn = document.createElement('button');
            customBtn.type = 'button';
            customBtn.className = 'flag-label-pill flag-label-pill-custom';
            customBtn.textContent = 'Custom\u2026';
            if (PREDEFINED_LABELS.indexOf(selectedLabel) === -1 && selectedLabel !== '') {
                customBtn.classList.add('selected');
                customBtn.textContent = selectedLabel;
            }
            customBtn.addEventListener('click', function (e) {
                e.stopPropagation();
                showCustomInput();
            });
            container.appendChild(customBtn);
        }

        function showCustomInput() {
            while (container.firstChild) {
                container.removeChild(container.firstChild);
            }
            var input = document.createElement('input');
            input.type = 'text';
            input.className = 'flag-label-custom-input';
            input.placeholder = 'Label name...';
            input.value = PREDEFINED_LABELS.indexOf(selectedLabel) === -1 ? selectedLabel : '';
            container.appendChild(input);
            input.focus();
            input.addEventListener('keydown', function (ev) {
                if (ev.key === 'Enter') {
                    ev.preventDefault();
                    var val = input.value.trim();
                    if (val) {
                        selectedLabel = val;
                        onSelect(val);
                    }
                    renderPills();
                }
                if (ev.key === 'Escape') {
                    renderPills();
                }
            });
            input.addEventListener('blur', function () {
                var val = input.value.trim();
                if (val) {
                    selectedLabel = val;
                    onSelect(val);
                }
                // Delay re-render so click on sibling elements (e.g. comment input)
                // lands before the DOM shifts from the layout change.
                setTimeout(renderPills, 150);
            });
        }

        renderPills();
        return container;
    }

    var toolbar = null;
    var currentFilepath = '';

    function initFlagToolbar() {
        var documentEl = document.getElementById('document');
        if (!documentEl) return;

        // Create the toolbar element
        toolbar = document.createElement('div');
        toolbar.className = 'flag-toolbar';

        var label = document.createElement('span');
        label.className = 'flag-toolbar-label';
        label.textContent = 'Add flag';
        toolbar.appendChild(label);

        var selectedLabel = 'Comment';
        var labelPicker = createLabelPicker(selectedLabel, function (pickedLabel) {
            selectedLabel = pickedLabel;
        });
        toolbar.appendChild(labelPicker);

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
            submitFlag(input.value.trim(), selectedText, selectedLabel);
        });

        // Submit on Enter key
        input.addEventListener('keydown', function (e) {
            if (e.key === 'Enter') {
                e.preventDefault();
                submitFlag(input.value.trim(), selectedText, selectedLabel);
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

    function submitFlag(comment, selectedText, label) {
        if (!comment || !selectedText || !currentFilepath) {
            hideToolbar();
            return;
        }

        var url = apiPrefix + '/flag/' + currentFilepath.split('/').map(encodeURIComponent).join('/');
        var body = JSON.stringify({
            comment: comment,
            selected_text: selectedText,
            label: label || 'Comment'
        });

        fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: body
        })
        .then(function (response) {
            if (response.ok) {
                suppressReloadBriefly();
                refreshFlagSidebar();
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
            var wsUrl = protocol + '//' + window.location.host + apiPrefix + '/ws';

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
                if (data === 'reload' && !suppressReload) {
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
        initSidebarToggle();
        initFlagSidebar();
        initFlagToolbar();
        initWebSocket();
    });

})();

// --- Docker: nav button visibility + dashboard ---
(function() {
    var navBtn = document.getElementById('docker-nav-btn');
    var list = document.getElementById('docker-list');
    var refreshBtn = document.getElementById('docker-refresh');
    var searchInput = document.getElementById('docker-search');

    // On non-dashboard pages, just probe Docker availability and show the nav button
    if (!list) {
        if (navBtn) {
            fetch('/api/docker/containers')
                .then(function(r) { if (r.ok) navBtn.style.display = ''; })
                .catch(function() {});
        }
        return;
    }

    // --- Dashboard page logic ---
    var allContainers = [];

    function clearChildren(el) {
        while (el.firstChild) el.removeChild(el.firstChild);
    }

    function shortStatus(status) {
        if (/^Up /i.test(status)) return 'running';
        if (/^Exited /i.test(status)) return 'exited';
        return status.split(' ')[0].toLowerCase();
    }

    function renderContainers(containers) {
        clearChildren(list);
        if (containers.length === 0) {
            var empty = document.createElement('p');
            empty.className = 'docker-empty';
            empty.textContent = allContainers.length === 0
                ? 'No running containers found.'
                : 'No containers match your filter.';
            list.appendChild(empty);
            return;
        }
        containers.forEach(function(c) {
            var card = document.createElement('a');
            card.className = 'docker-card';
            card.href = '/docker/' + encodeURIComponent(c.name);

            var top = document.createElement('div');
            top.className = 'docker-card-top';

            var icon = document.createElement('span');
            icon.className = 'docker-card-icon';
            icon.textContent = '\uD83D\uDC33';
            top.appendChild(icon);

            var name = document.createElement('span');
            name.className = 'docker-card-name';
            name.textContent = c.name;
            top.appendChild(name);

            var status = document.createElement('span');
            status.className = 'docker-card-status';
            status.textContent = shortStatus(c.status);
            top.appendChild(status);

            card.appendChild(top);

            var meta = document.createElement('div');
            meta.className = 'docker-card-meta';

            var imageItem = document.createElement('span');
            imageItem.className = 'docker-card-meta-item';
            var imageLabel = document.createElement('span');
            imageLabel.className = 'docker-card-meta-label';
            imageLabel.textContent = 'image';
            imageItem.appendChild(imageLabel);
            var imageVal = document.createElement('span');
            imageVal.textContent = c.image;
            imageItem.appendChild(imageVal);
            meta.appendChild(imageItem);

            if (c.workdir && c.workdir !== '/') {
                var wdItem = document.createElement('span');
                wdItem.className = 'docker-card-meta-item';
                var wdLabel = document.createElement('span');
                wdLabel.className = 'docker-card-meta-label';
                wdLabel.textContent = 'workdir';
                wdItem.appendChild(wdLabel);
                var wdVal = document.createElement('span');
                wdVal.textContent = c.workdir;
                wdItem.appendChild(wdVal);
                meta.appendChild(wdItem);
            }

            card.appendChild(meta);
            list.appendChild(card);
        });
    }

    function filterContainers() {
        var query = (searchInput ? searchInput.value : '').toLowerCase().trim();
        if (!query) {
            renderContainers(allContainers);
            return;
        }
        var filtered = allContainers.filter(function(c) {
            return c.name.toLowerCase().indexOf(query) !== -1
                || c.image.toLowerCase().indexOf(query) !== -1;
        });
        renderContainers(filtered);
    }

    function fetchContainers() {
        fetch('/api/docker/containers')
            .then(function(r) {
                if (r.ok) return r.json();
                throw new Error('Docker not available');
            })
            .then(function(data) {
                allContainers = data;
                filterContainers();
            })
            .catch(function() {
                clearChildren(list);
                var err = document.createElement('p');
                err.className = 'docker-empty';
                err.textContent = 'Could not connect to Docker.';
                list.appendChild(err);
            });
    }

    if (refreshBtn) {
        refreshBtn.addEventListener('click', fetchContainers);
    }

    if (searchInput) {
        searchInput.addEventListener('input', filterContainers);
    }

    fetchContainers();
})();
