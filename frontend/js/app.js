/**
 * Deloxide - Deadlock Detection Visualization
 * 
 * This file contains the main JavaScript code for the deadlock detection
 * visualization tool. It manages the D3.js visualization, UI interactions,
 * and animation controls.
 */

// Global variables for visualization
let logData = [];
let graphStateData = [];
let currentStep = 1;
let nodes = [];
let links = [];
let svg, linkGroup, nodeGroup, tooltip, simulation;
let currentScenario = null;
let animationInterval = null;
let isPlaying = false;

// Theme management
const themeToggle = document.getElementById('theme-toggle');
const themeIcon = document.getElementById('theme-icon');
const prefersDarkScheme = window.matchMedia('(prefers-color-scheme: dark)');

// Check for saved theme preference or use the system preference
const getCurrentTheme = () => {
    const savedTheme = localStorage.getItem('theme');
    if (savedTheme) {
        return savedTheme;
    }
    return prefersDarkScheme.matches ? 'dark' : 'light';
};

// Apply the current theme
const applyTheme = (theme) => {
    document.documentElement.setAttribute('data-theme', theme);
    
    if (themeIcon) {
        if (theme === 'dark') {
            themeIcon.className = 'fas fa-sun';
            themeToggle.setAttribute('aria-label', 'Switch to light mode');
            themeToggle.querySelector('span').textContent = 'Light Mode';
        } else {
            themeIcon.className = 'fas fa-moon';
            themeToggle.setAttribute('aria-label', 'Switch to dark mode');
            themeToggle.querySelector('span').textContent = 'Dark Mode';
        }
    }
};

// Toggle between light and dark themes
const toggleTheme = () => {
    const currentTheme = getCurrentTheme();
    const newTheme = currentTheme === 'light' ? 'dark' : 'light';
    
    localStorage.setItem('theme', newTheme);
    applyTheme(newTheme);
};

// Upload functionality
const initUploadFeature = () => {
    const uploadBtn = document.getElementById('upload-btn');
    const uploadModal = document.getElementById('upload-modal');
    const closeBtn = uploadModal.querySelector('.modal-close');
    const dropArea = document.getElementById('drop-area');
    const fileInput = document.getElementById('file-input');
    const fileSelectBtn = document.getElementById('file-select-btn');
    const uploadList = document.getElementById('upload-list');
    const jsonPreview = document.getElementById('json-preview');
    const jsonContent = document.getElementById('json-content');
    const shareBtn = document.getElementById('share-btn');
    
    // Share functionality
    if (shareBtn) {
        shareBtn.addEventListener('click', openShareModal);
    }
    
    // Open modal when upload button is clicked
    uploadBtn.addEventListener('click', () => {
        uploadModal.style.display = 'flex';
    });
    
    // Close modal
    closeBtn.addEventListener('click', () => {
        uploadModal.style.display = 'none';
    });
    
    // Close modal when clicking outside
    window.addEventListener('click', (e) => {
        if (e.target === uploadModal) {
            uploadModal.style.display = 'none';
        }
    });
    
    // Open file dialog when button is clicked
    fileSelectBtn.addEventListener('click', () => {
        fileInput.click();
    });
    
    // Handle file selection
    fileInput.addEventListener('change', () => {
        handleFiles(fileInput.files);
    });
    
    // Prevent default drag behaviors
    ['dragenter', 'dragover', 'dragleave', 'drop'].forEach(eventName => {
        dropArea.addEventListener(eventName, preventDefaults, false);
    });
    
    function preventDefaults(e) {
        e.preventDefault();
        e.stopPropagation();
    }
    
    // Highlight drop area when dragging over it
    ['dragenter', 'dragover'].forEach(eventName => {
        dropArea.addEventListener(eventName, highlight, false);
    });
    
    ['dragleave', 'drop'].forEach(eventName => {
        dropArea.addEventListener(eventName, unhighlight, false);
    });
    
    function highlight() {
        dropArea.classList.add('highlight');
    }
    
    function unhighlight() {
        dropArea.classList.remove('highlight');
    }
    
    // Handle dropped files
    dropArea.addEventListener('drop', (e) => {
        const dt = e.dataTransfer;
        const files = dt.files;
        handleFiles(files);
    });
    
    // Process the files
    function handleFiles(files) {
        // Convert FileList to array for easier handling
        const filesArray = Array.from(files);
        
        // Filter only JSON files
        const jsonFiles = filesArray.filter(file => file.name.endsWith('.json'));
        
        if (jsonFiles.length === 0) {
            alert('Please upload JSON files only');
            return;
        }
        
        // Clear previous uploads
        uploadList.innerHTML = '';
        
        // Process each file
        jsonFiles.forEach(file => {
            // Show in upload list
            const fileItem = document.createElement('div');
            fileItem.className = 'upload-item';
            
            const fileName = document.createElement('span');
            fileName.className = 'upload-item-name';
            fileName.textContent = file.name;
            
            const fileSize = document.createElement('span');
            fileSize.className = 'upload-item-size';
            fileSize.textContent = formatFileSize(file.size);
            
            const viewBtn = document.createElement('button');
            viewBtn.className = 'upload-item-view';
            viewBtn.textContent = 'View';
            viewBtn.addEventListener('click', () => {
                readAndPreviewJSON(file);
            });
            
            const loadBtn = document.createElement('button');
            loadBtn.className = 'upload-item-load';
            loadBtn.textContent = 'Load';
            loadBtn.addEventListener('click', () => {
                loadScenarioFromFile(file);
            });
            
            fileItem.appendChild(fileName);
            fileItem.appendChild(fileSize);
            fileItem.appendChild(viewBtn);
            fileItem.appendChild(loadBtn);
            uploadList.appendChild(fileItem);
            
            // Auto-preview the first file
            if (uploadList.children.length === 1) {
                readAndPreviewJSON(file);
            }
        });
    }
    
    // Format file size for display
    function formatFileSize(bytes) {
        if (bytes < 1024) return bytes + ' bytes';
        else if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
        else return (bytes / 1048576).toFixed(1) + ' MB';
    }
    
    // Read and show JSON content
    function readAndPreviewJSON(file) {
        const reader = new FileReader();
        
        reader.onload = function(e) {
            try {
                const jsonData = JSON.parse(e.target.result);
                const formattedJSON = JSON.stringify(jsonData, null, 2);
                jsonContent.textContent = formattedJSON;
                jsonPreview.style.display = 'block';
                
                // Validate if this is a proper deadlock log
                if (validateDeadlockLog(jsonData)) {
                    console.log('Valid deadlock log file loaded');
                } else {
                    console.warn('The uploaded file does not appear to be a valid deadlock log');
                    alert('Warning: The file does not appear to be a valid deadlock log file. It may not display correctly.');
                }
            } catch (error) {
                jsonContent.textContent = 'Error parsing JSON: ' + error.message;
                jsonPreview.style.display = 'block';
            }
        };
        
        reader.onerror = function() {
            jsonContent.textContent = 'Error reading file';
            jsonPreview.style.display = 'block';
        };
        
        reader.readAsText(file);
    }
    
    // Load scenario from uploaded file
    function loadScenarioFromFile(file) {
        const reader = new FileReader();
        
        reader.onload = function(e) {
            try {
                const jsonData = JSON.parse(e.target.result);
                
                // Validate if this is a proper deadlock log
                if (validateDeadlockLog(jsonData)) {
                    // Close the modal
                    uploadModal.style.display = 'none';
                    
                    // Process the scenario data
                    resetVisualization();
                    currentScenario = jsonData;
                    logData = jsonData.logs;
                    graphStateData = jsonData.graph_state;
                    
                    // Update scenario information
                    updateScenarioInfo(jsonData);
                    
                    // Initialize visualization
                    currentStep = 1;
                    
                    // Show loading state while we initialize
                    document.getElementById('loading').style.display = 'block';
                    document.getElementById('loading').innerHTML = '<div class="spinner"></div><p>Loading visualization...</p>';
                    
                    // Show share button since we have data loaded
                    if (shareBtn) {
                        shareBtn.style.display = 'flex';
                    }
                    
                    // Initialize after a brief delay to allow the UI to update
                    setTimeout(() => {
                        initVisualization();
                        
                        // Hide loading message and show visualization elements
                        document.getElementById('loading').style.display = 'none';
                        document.getElementById('graph').style.display = 'block';
                        document.getElementById('step-info').style.display = 'block';
                        document.getElementById('wait-graph').style.display = 'block';
                        document.getElementById('timeline').style.display = 'block';
                        
                        // Initialize timeline
                        initTimeline();
                        
                        // Update visualization for the first step
                        updateVisualization();
                    }, 100);
                } else {
                    alert('Error: The file is not a valid deadlock log file. Please upload a properly formatted file.');
                }
            } catch (error) {
                alert('Error loading file: ' + error.message);
            }
        };
        
        reader.onerror = function() {
            alert('Error reading file.');
        };
        
        reader.readAsText(file);
    }
};

// Share functionality
function initShareFeature() {
    const shareModal = document.getElementById('share-modal');
    const closeBtns = shareModal.querySelectorAll('.modal-close');
    const copyBtn = document.getElementById('copy-link-btn');
    const shareLinkInput = document.getElementById('share-link');
    const copyStatus = document.getElementById('copy-status');
    
    // Close modal when clicking the X button
    closeBtns.forEach(btn => {
        btn.addEventListener('click', () => {
            shareModal.style.display = 'none';
            copyStatus.style.display = 'none';
        });
    });
    
    // Close modal when clicking outside
    window.addEventListener('click', (e) => {
        if (e.target === shareModal) {
            shareModal.style.display = 'none';
            copyStatus.style.display = 'none';
        }
    });
    
    // Copy link to clipboard
    if (copyBtn) {
        copyBtn.addEventListener('click', () => {
            // Select the text first
            shareLinkInput.select();
            
            // Try to use the modern Clipboard API
            if (navigator.clipboard && window.isSecureContext) {
                navigator.clipboard.writeText(shareLinkInput.value)
                    .then(() => {
                        // Show success message
                        showCopySuccess();
                    })
                    .catch(err => {
                        console.error('Could not copy text using Clipboard API:', err);
                        // Fall back to execCommand
                        fallbackCopy();
                    });
            } else {
                // Fall back to execCommand for older browsers
                fallbackCopy();
            }
        });
    }
    
    function fallbackCopy() {
        try {
            const successful = document.execCommand('copy');
            if (successful) {
                showCopySuccess();
            } else {
                console.error('Fallback: Unable to copy');
                alert('Unable to copy to clipboard. Please select the text and copy manually.');
            }
        } catch (err) {
            console.error('Fallback: Unable to copy', err);
            alert('Unable to copy to clipboard. Please select the text and copy manually.');
        }
    }
    
    function showCopySuccess() {
        // Show success message
        copyStatus.style.display = 'flex';
        
        // Hide after 3 seconds
        setTimeout(() => {
            copyStatus.style.display = 'none';
        }, 3000);
    }
    
    // Check if there's a shared scenario in the URL
    checkForSharedScenario();
}

// Open share modal and generate shareable link
function openShareModal() {
    const shareModal = document.getElementById('share-modal');
    const shareLinkInput = document.getElementById('share-link');
    
    if (!currentScenario) {
        alert('No scenario is currently loaded. Please upload a scenario first.');
        return;
    }
    
    try {
        console.log('Preparing to share scenario:', currentScenario.title);
        
        // Create a compressed version of the current scenario
        const scenarioString = JSON.stringify(currentScenario);
        console.log('Original data size:', scenarioString.length, 'bytes');
        
        // Compress the data
        const compressedData = LZString.compressToEncodedURIComponent(scenarioString);
        console.log('Compressed data size:', compressedData.length, 'bytes');
        
        // Generate the full URL with the compressed data
        const currentUrl = window.location.href.split('?')[0]; // Remove any existing query parameters
        const shareUrl = `${currentUrl}?data=${compressedData}&step=${currentStep}`;
        
        console.log('Share URL generated, length:', shareUrl.length);
        
        // Set the input value
        shareLinkInput.value = shareUrl;
        
        // Show the modal
        shareModal.style.display = 'flex';
    } catch (error) {
        console.error('Error generating share link:', error);
        alert('Error generating share link: ' + error.message);
    }
}

// Check for shared scenario in URL parameters
function checkForSharedScenario() {
    const urlParams = new URLSearchParams(window.location.search);
    const encodedData = urlParams.get('data');
    const step = urlParams.get('step');
    
    if (encodedData) {
        try {
            console.log('Found shared data in URL, processing...');
            
            // Show loading state
            document.getElementById('loading').style.display = 'block';
            document.getElementById('loading').innerHTML = '<div class="spinner"></div><p>Loading shared visualization...</p>';
            
            // Decode the data
            console.log('Compressed data size:', encodedData.length, 'bytes');
            const decompressedData = LZString.decompressFromEncodedURIComponent(encodedData);
            
            if (!decompressedData) {
                throw new Error('Failed to decompress data');
            }
            
            console.log('Decompressed data size:', decompressedData.length, 'bytes');
            const scenarioData = JSON.parse(decompressedData);
            console.log('Successfully parsed JSON data');
            
            // Validate the data
            if (validateDeadlockLog(scenarioData)) {
                console.log('Valid deadlock log format detected');
                
                // Process the scenario data
                resetVisualization();
                currentScenario = scenarioData;
                logData = scenarioData.logs;
                graphStateData = scenarioData.graph_state;
                
                // Update scenario information
                updateScenarioInfo(scenarioData);
                
                // Set step if provided
                currentStep = step ? parseInt(step) : 1;
                if (isNaN(currentStep) || currentStep < 1 || currentStep > logData.length) {
                    currentStep = 1;
                }
                console.log('Setting to step:', currentStep);
                
                // Show share button
                const shareBtn = document.getElementById('share-btn');
                if (shareBtn) {
                    shareBtn.style.display = 'flex';
                }
                
                // Initialize visualization
                setTimeout(() => {
                    initVisualization();
                    
                    // Hide loading message and show visualization elements
                    document.getElementById('loading').style.display = 'none';
                    document.getElementById('graph').style.display = 'block';
                    document.getElementById('step-info').style.display = 'block';
                    document.getElementById('wait-graph').style.display = 'block';
                    document.getElementById('timeline').style.display = 'block';
                    
                    // Initialize timeline
                    initTimeline();
                    
                    // Update visualization with the specified step
                    updateVisualization();
                    
                    console.log('Shared visualization loaded successfully');
                }, 100);
            } else {
                console.error('Invalid deadlock log format in shared data');
                document.getElementById('loading').innerHTML = `
                    <div class="error-message">
                        <i class="fas fa-exclamation-triangle"></i> 
                        The shared visualization data is invalid or corrupted.
                    </div>`;
            }
        } catch (error) {
            console.error('Error loading shared scenario:', error);
            document.getElementById('loading').innerHTML = `
                <div class="error-message">
                    <i class="fas fa-exclamation-triangle"></i> 
                    Error loading shared visualization: ${error.message}
                </div>`;
        }
    }
}

// Helper function to validate deadlock log structure
function validateDeadlockLog(json) {
    return (
        json && 
        typeof json === 'object' && 
        json.title && 
        json.description && 
        Array.isArray(json.logs) && 
        json.logs.length > 0 &&
        Array.isArray(json.graph_state) &&
        json.graph_state.length > 0
    );
}

// Update scenario info in the UI
function updateScenarioInfo(scenarioData) {
    const scenarioTitle = document.createElement('h2');
    scenarioTitle.textContent = scenarioData.title;
    
    const scenarioDesc = document.createElement('p');
    scenarioDesc.className = 'scenario-description';
    scenarioDesc.textContent = scenarioData.description;
    
    const scenarioInfo = document.createElement('div');
    scenarioInfo.id = 'scenario-info';
    scenarioInfo.className = 'scenario-info';
    scenarioInfo.appendChild(scenarioTitle);
    scenarioInfo.appendChild(scenarioDesc);
    
    // Add to DOM
    const mainElement = document.querySelector('main');
    const existingInfo = document.getElementById('scenario-info');
    const controlsElement = document.getElementById('controls');
    
    if (existingInfo) {
        mainElement.replaceChild(scenarioInfo, existingInfo);
    } else {
        mainElement.insertBefore(scenarioInfo, controlsElement);
    }
}

// Initialize theme
const initTheme = () => {
    const currentTheme = getCurrentTheme();
    applyTheme(currentTheme);
    
    if (themeToggle) {
        themeToggle.addEventListener('click', toggleTheme);
    }
    
    // Listen for system theme changes
    prefersDarkScheme.addEventListener('change', (e) => {
        if (!localStorage.getItem('theme')) {
            applyTheme(e.matches ? 'dark' : 'light');
        }
    });
};

/**
 * Check if D3.js is available
 */
function checkD3Availability() {
    if (typeof d3 === 'undefined') {
        console.error('D3.js is not loaded');
        return false;
    }
    return true;
}

/**
 * Load scenario list and populate dropdown
 */
async function loadScenarioList() {
    // Show instruction message since we're not loading built-in scenarios anymore
    const loadingElement = document.getElementById('loading');
    if (loadingElement) {
        loadingElement.innerHTML = `
            <div class="welcome-message">
                <i class="fas fa-upload"></i>
                <h2>No Scenario Loaded</h2>
                <p>Click the "Upload" button in the header to load a deadlock log file.</p>
            </div>`;
    }
}

/**
 * Reset the visualization
 */
function resetVisualization() {
    // Stop any running animation
    if (isPlaying) {
        togglePlay();
    }
    
    // Clear existing visualization
    if (svg) {
        svg.remove();
        svg = null;
    }
    
    // Clear timeline
    const timelineElement = document.getElementById('timeline');
    if (timelineElement) {
        // Keep the timeline-line and timeline-marker, remove all events
        const timelineLine = document.getElementById('timeline-line');
        const timelineMarker = document.getElementById('timeline-marker');
        
        while (timelineElement.firstChild) {
            timelineElement.removeChild(timelineElement.firstChild);
        }
        
        if (timelineLine && timelineMarker) {
            timelineElement.appendChild(timelineLine);
            timelineElement.appendChild(timelineMarker);
        }
    }
    
    // Reset variables
    currentStep = 1;
    nodes = [];
    links = [];
}

/**
 * Initialize the D3.js visualization
 */
function initVisualization() {
    console.log('Initializing visualization...');
    
    // Make sure the graph container is visible
    const graphContainer = document.getElementById('graph');
    graphContainer.style.display = 'block';
    
    // Set dimensions based on container
    const width = graphContainer.clientWidth;
    const height = graphContainer.clientHeight;
    
    console.log('Using dimensions:', width, 'x', height);
    
    svg = d3.select("#graph")
        .append("svg")
        .attr("width", "100%")
        .attr("height", "100%")
        .attr("viewBox", `0 0 ${width} ${height}`)
        .attr("preserveAspectRatio", "xMidYMid meet");
        
    tooltip = d3.select("#graph")
        .append("div")
        .attr("class", "tooltip")
        .style("opacity", 0);
    
    // Add arrow markers for the links
    svg.append("defs").append("marker")
        .attr("id", "arrowhead")
        .attr("viewBox", "0 -5 10 10")
        .attr("refX", 35)
        .attr("refY", 0)
        .attr("markerWidth", 6)
        .attr("markerHeight", 6)
        .attr("orient", "auto")
        .append("path")
        .attr("d", "M0,-5L10,0L0,5")
        .attr("fill", "var(--neutral-color)");
    
    try {
        // Make sure we have graph data
        if (!graphStateData || graphStateData.length === 0) {
            throw new Error("No graph state data available");
        }
        
        // Initialize with the nodes from the first step
        nodes = JSON.parse(JSON.stringify(graphStateData[0].nodes));
        console.log('Initial nodes:', nodes);
        
        // Set initial positions to prevent initial animation from being too wild
        nodes.forEach((node, index) => {
            // Distribute nodes more evenly across the graph
            const angleStep = (2 * Math.PI) / nodes.length;
            const radius = Math.min(width, height) * 0.3; // Use 30% of the smaller dimension
            
            const angle = angleStep * index;
            
            // Set positions in a circle formation
            node.x = width/2 + radius * Math.cos(angle);
            node.y = height/2 + radius * Math.sin(angle);
            
            // Set initial fixed positions (user can later move them)
            node.fx = node.x;
            node.fy = node.y;
        });
        
        // Create the link and node groups
        linkGroup = svg.append("g").attr("class", "links");
        nodeGroup = svg.append("g").attr("class", "nodes");
        
        // Set up force simulation with fixed bounds
        simulation = d3.forceSimulation(nodes)
            .force("link", d3.forceLink()
                .id(d => d.id)
                .distance(150))
            .force("charge", d3.forceManyBody().strength(-700))
            .force("center", d3.forceCenter(width / 2, height / 2))
            .force("x", d3.forceX(width / 2).strength(0.1))
            .force("y", d3.forceY(height / 2).strength(0.1))
            .force("collision", d3.forceCollide().radius(60)) // Prevent node overlap
            .on("tick", ticked);
        
        // Create node elements
        let nodeElements = nodeGroup.selectAll(".node")
            .data(nodes)
            .enter()
            .append("g")
            .attr("class", d => `node ${d.type}`)
            .call(d3.drag()
                .on("start", dragstarted)
                .on("drag", dragged)
                .on("end", dragended));
                
        // Add circles to node groups
        nodeElements.append("circle")
            .attr("r", 25);
            
        // Add labels to nodes
        nodeElements.append("text")
            .attr("text-anchor", "middle")
            .attr("dy", 5)
            .text(d => d.id)
            .style("font-weight", "bold");
        
        // Add tooltips to nodes
        nodeElements.on("mouseover", function(event, d) {
            tooltip.transition()
                .duration(200)
                .style("opacity", .9);
            tooltip.html(d.name)
                .style("left", (event.pageX - document.getElementById('graph').getBoundingClientRect().left + 10) + "px")
                .style("top", (event.pageY - document.getElementById('graph').getBoundingClientRect().top - 28) + "px");
        })
        .on("mouseout", function() {
            tooltip.transition()
                .duration(500)
                .style("opacity", 0);
        });
        
        console.log('Visualization initialized successfully');
    } catch (error) {
        console.error('Error initializing visualization:', error);
        document.getElementById('loading').style.display = 'block';
        document.getElementById('loading').innerHTML = `<div class="error-message"><i class="fas fa-exclamation-triangle"></i> Error initializing visualization: ${error.message}</div>`;
    }
}

/**
 * Update the visualization based on current step
 */
function updateVisualization() {
    console.log('Updating visualization for step', currentStep);
    
    if (!graphStateData || graphStateData.length === 0) {
        console.error('No graph state data available');
        return;
    }
    
    try {
        // Get the current graph state
        const currentGraphState = graphStateData[currentStep - 1];
        console.log('Current graph state:', currentGraphState);
        
        // Update nodes for this step (make sure nodes are up to date)
        // This will ensure new nodes can be added in later steps
        const nodeIds = new Set(nodes.map(n => n.id));
        currentGraphState.nodes.forEach(node => {
            if (!nodeIds.has(node.id)) {
                // Add missing nodes
                const newNode = Object.assign({}, node);
                // Set initial position close to center to avoid animation jumping
                const graphContainer = document.getElementById('graph');
                newNode.x = graphContainer.clientWidth / 2;
                newNode.y = graphContainer.clientHeight / 2;
                nodes.push(newNode);
            }
        });
        
        // Rebuild node display elements if needed
        const existingNodeElements = nodeGroup.selectAll(".node")
            .data(nodes, d => d.id);
            
        // Add new nodes if they appeared in this step
        const nodeEnter = existingNodeElements.enter()
            .append("g")
            .attr("class", d => `node ${d.type}`)
            .call(d3.drag()
                .on("start", dragstarted)
                .on("drag", dragged)
                .on("end", dragended));
                
        // Add circles to new node groups
        nodeEnter.append("circle")
            .attr("r", 25);
            
        // Add labels to new nodes
        nodeEnter.append("text")
            .attr("text-anchor", "middle")
            .attr("dy", 5)
            .text(d => d.id)
            .style("font-weight", "bold");
        
        // Add tooltips to new nodes
        nodeEnter.on("mouseover", function(event, d) {
            tooltip.transition()
                .duration(200)
                .style("opacity", .9);
            tooltip.html(d.name)
                .style("left", (event.pageX - document.getElementById('graph').getBoundingClientRect().left + 10) + "px")
                .style("top", (event.pageY - document.getElementById('graph').getBoundingClientRect().top - 28) + "px");
        })
        .on("mouseout", function() {
            tooltip.transition()
                .duration(500)
                .style("opacity", 0);
        });
        
        // Convert links to the right format for D3
        links = currentGraphState.links.map(link => {
            return {
                source: typeof link.source === 'string' ? 
                       nodes.find(n => n.id === link.source) : link.source,
                target: typeof link.target === 'string' ? 
                       nodes.find(n => n.id === link.target) : link.target,
                type: link.type
            };
        });
        
        // Update the force simulation with new links but don't release fixed positions
        simulation.nodes(nodes);
        simulation.force("link").links(links);
        
        // Restart simulation with a gentle alpha to adjust positions
        simulation.alpha(0.1).restart();
        
        // Draw links
        const linkElements = linkGroup.selectAll(".link")
            .data(links, d => `${d.source.id || d.source}-${d.target.id || d.target}`);
        
        console.log('Link elements:', linkElements.size());
        
        linkElements.exit().remove();
        
        const enterLinks = linkElements.enter()
            .append("line")
            .attr("class", d => `link ${d.type}`)
            .attr("marker-end", "url(#arrowhead)");
        
        // Merge the enter and update selection
        enterLinks.merge(linkElements)
            .attr("class", d => `link ${d.type}`);
        
        // Update step information
        updateStepInfo();
        
        // Update timeline marker
        updateTimelineMarker();
        
        // Update buttons state
        document.getElementById("prev-btn").disabled = currentStep === 1;
        document.getElementById("next-btn").disabled = currentStep === logData.length;
        
        // Highlight current event in timeline
        document.querySelectorAll('.timeline-event').forEach(event => {
            if (parseInt(event.getAttribute('data-step')) === currentStep) {
                event.style.transform = 'translate(-50%, -50%) scale(1.4)';
                event.style.boxShadow = '0 0 8px var(--primary-color)';
            } else {
                event.style.transform = 'translate(-50%, -50%)';
                event.style.boxShadow = '';
            }
        });
    } catch (error) {
        console.error('Error updating visualization:', error);
    }
}

/**
 * Update step information in the info panel
 */
function updateStepInfo() {
    const stepInfoElement = document.getElementById('step-info');
    const waitGraphElement = document.getElementById('wait-graph');
    
    if (stepInfoElement) {
        // Get the log entry for current step
        const logEntry = logData[currentStep - 1];
        
        // Create main step info
        let stepInfoContent = `<h3>Step ${logEntry.step}: ${logEntry.type.toUpperCase()}</h3>`;
        stepInfoContent += `<p>${logEntry.description}</p>`;
        
        if (logEntry.code_reference) {
            stepInfoContent += `<p><strong>Code Reference:</strong> <code class="code-reference">${logEntry.code_reference}</code></p>`;
        }
        
        // Add additional details based on event type
        if (logEntry.type === 'attempt' || logEntry.type === 'acquired' || logEntry.type === 'released') {
            stepInfoContent += `<p><strong>Thread:</strong> ${logEntry.thread_name} (ID: ${logEntry.thread_id})<br>`;
            stepInfoContent += `<strong>Resource:</strong> ${logEntry.resource_name} (ID: ${logEntry.resource_id})</p>`;
        }
        
        // Add timestamp
        const date = new Date(logEntry.timestamp);
        stepInfoContent += `<p><small>Timestamp: ${date.toLocaleString()}</small></p>`;
        
        // Set content
        stepInfoElement.innerHTML = stepInfoContent;
    }
    
    // Update wait-for graph information if applicable
    if (waitGraphElement) {
        const logEntry = logData[currentStep - 1];
        
        // If this is a deadlock event, show detailed information
        if (logEntry.type === 'deadlock' && logEntry.deadlock_details) {
            waitGraphElement.style.display = 'block';
            
            // Construct wait-for graph explanation
            let waitGraphContent = `<h3>Wait-for Graph</h3><div id="wait-graph-content">`;
            
            waitGraphContent += `<p>A cycle has been detected in the wait-for graph: `;
            
            // Format the cycle
            const cycle = logEntry.deadlock_details.thread_cycle;
            waitGraphContent += cycle.map(t => `Thread ${t}`).join(' → ') + ` → Thread ${cycle[0]}</p>`;
            
            waitGraphContent += `<p><strong>Resources involved:</strong></p><ul>`;
            logEntry.deadlock_details.thread_waiting_for_locks.forEach(item => {
                const threadLog = logData.find(log => 
                    log.thread_id === item.thread_id && 
                    log.resource_id === item.lock_id && 
                    log.type === 'attempt'
                );
                
                if (threadLog) {
                    waitGraphContent += `<li>Thread ${item.thread_id} (${threadLog.thread_name}) is waiting for Resource ${item.lock_id} (${threadLog.resource_name})</li>`;
                } else {
                    waitGraphContent += `<li>Thread ${item.thread_id} is waiting for Resource ${item.lock_id}</li>`;
                }
            });
            waitGraphContent += `</ul></div>`;
            
            waitGraphElement.innerHTML = waitGraphContent;
        } else if (logEntry.wait_for_edge) {
            waitGraphElement.style.display = 'block';
            
            // Show simple wait-for edge
            const { from, to } = logEntry.wait_for_edge;
            
            let waitGraphContent = `<h3>Wait-for Graph</h3><div id="wait-graph-content">`;
            waitGraphContent += `<p>Thread ${from} is waiting for a resource held by Thread ${to}.</p>`;
            waitGraphContent += `<p>This creates an edge in the wait-for graph from Thread ${from} to Thread ${to}.</p>`;
            waitGraphContent += `</div>`;
            
            waitGraphElement.innerHTML = waitGraphContent;
        } else {
            waitGraphElement.style.display = 'none';
        }
    }
}

/**
 * Update the timeline marker position
 */
function updateTimelineMarker() {
    const timelineElement = document.getElementById('timeline');
    const markerElement = document.getElementById('timeline-marker');
    
    if (timelineElement && markerElement && logData.length > 0) {
        // Calculate marker position
        const totalWidth = timelineElement.clientWidth - 40; // 20px padding on each side
        const position = 20 + (totalWidth * (currentStep - 1) / (logData.length - 1));
        
        markerElement.style.left = `${position}px`;
    }
}

/**
 * D3.js force simulation tick function
 */
function ticked() {
    // Update link positions
    linkGroup.selectAll(".link")
        .attr("x1", d => d.source.x)
        .attr("y1", d => d.source.y)
        .attr("x2", d => d.target.x)
        .attr("y2", d => d.target.y);
    
    // Update node positions
    nodeGroup.selectAll(".node")
        .attr("transform", d => `translate(${d.x}, ${d.y})`);
        
    // Keep nodes within bounds of the svg container
    const svgBounds = svg.node().getBoundingClientRect();
    const padding = 30; // Padding to prevent nodes from touching the edge
    
    nodes.forEach(d => {
        // Calculate safe bounds based on the SVG container
        const minX = padding;
        const maxX = svgBounds.width - padding;
        const minY = padding;
        const maxY = svgBounds.height - padding;
        
        // Enforce the bounds
        d.x = Math.max(minX, Math.min(maxX, d.x));
        d.y = Math.max(minY, Math.min(maxY, d.y));
    });
}

/**
 * D3.js drag functions
 */
function dragstarted(event, d) {
    if (!event.active) simulation.alphaTarget(0.3).restart();
    d.fx = d.x;
    d.fy = d.y;
}

function dragged(event, d) {
    d.fx = event.x;
    d.fy = event.y;
}

function dragended(event, d) {
    if (!event.active) simulation.alphaTarget(0);
    // Keep the node fixed where it was dragged
    // d.fx = null;
    // d.fy = null;
}

/**
 * Initialize the timeline with events
 */
function initTimeline() {
    const timelineElement = document.getElementById('timeline');
    
    if (timelineElement && logData.length > 0) {
        // Clear existing events
        const timelineLine = document.getElementById('timeline-line');
        const timelineMarker = document.getElementById('timeline-marker');
        
        while (timelineElement.firstChild) {
            timelineElement.removeChild(timelineElement.firstChild);
        }
        
        // Add back the line and marker
        timelineElement.appendChild(document.createElement('div')).id = 'timeline-line';
        timelineElement.appendChild(document.createElement('div')).id = 'timeline-marker';
        
        // Calculate event positions
        const totalWidth = timelineElement.clientWidth - 40; // 20px padding on each side
        
        // Create events
        logData.forEach((event, index) => {
            const position = 20 + (totalWidth * index / (logData.length - 1));
            
            const eventElement = document.createElement('div');
            eventElement.className = `timeline-event ${event.type}`;
            eventElement.style.left = `${position}px`;
            eventElement.setAttribute('data-step', event.step);
            eventElement.setAttribute('title', `Step ${event.step}: ${event.type}`);
            
            eventElement.addEventListener('click', () => {
                currentStep = event.step;
                updateVisualization();
            });
            
            timelineElement.appendChild(eventElement);
        });
    }
}

/**
 * Toggle play/pause of animation
 */
function togglePlay() {
    const playBtn = document.getElementById("play-btn");
    const playBtnText = playBtn.querySelector('span');
    const playBtnIcon = playBtn.querySelector('i');
    
    if (isPlaying) {
        // Stop playback
        clearInterval(animationInterval);
        isPlaying = false;
        playBtnText.textContent = 'Play Animation';
        playBtnIcon.className = 'fas fa-play';
    } else {
        // Start playback
        playBtnText.textContent = 'Stop Animation';
        playBtnIcon.className = 'fas fa-stop';
        isPlaying = true;
        
        // Start from the beginning if at the end
        if (currentStep >= logData.length) {
            currentStep = 1;
        }
        
        updateVisualization();
        
        let step = currentStep + 1;
        animationInterval = setInterval(() => {
            if (step > logData.length) {
                clearInterval(animationInterval);
                isPlaying = false;
                playBtnText.textContent = 'Play Animation';
                playBtnIcon.className = 'fas fa-play';
                return;
            }
            
            currentStep = step;
            updateVisualization();
            step++;
        }, 1000);
    }
}

/**
 * Setup event listeners for UI controls
 */
function setupEventListeners() {
    document.getElementById("prev-btn").addEventListener("click", () => {
        if (currentStep > 1) {
            currentStep--;
            updateVisualization();
        }
    });
    
    document.getElementById("next-btn").addEventListener("click", () => {
        if (currentStep < logData.length) {
            currentStep++;
            updateVisualization();
        }
    });
    
    document.getElementById("play-btn").addEventListener("click", togglePlay);
    
    document.getElementById("reset-btn").addEventListener("click", () => {
        currentStep = 1;
        updateVisualization();
    });
    
    // Add keyboard navigation
    document.addEventListener('keydown', (e) => {
        // Left arrow key
        if (e.keyCode === 37) {
            if (currentStep > 1) {
                currentStep--;
                updateVisualization();
            }
        }
        // Right arrow key
        else if (e.keyCode === 39) {
            if (currentStep < logData.length) {
                currentStep++;
                updateVisualization();
            }
        }
        // Space key to play/pause
        else if (e.keyCode === 32 && !e.target.matches('button, input')) {
            e.preventDefault();
            togglePlay();
        }
        // R key to reset
        else if (e.keyCode === 82 && !e.target.matches('input, textarea')) {
            document.getElementById("reset-btn").click();
        }
    });
    
    // Handle window resize
    window.addEventListener('resize', () => {
        // Redraw timeline
        initTimeline();
        
        // Update timeline marker
        updateTimelineMarker();
    });
}

/**
 * Initialize the application
 */
function initApp() {
    console.log('Initializing application...');
    
    // Check for shared data in URL first
    const urlParams = new URLSearchParams(window.location.search);
    const hasSharedData = urlParams.has('data');
    
    // Initialize theme
    initTheme();
    
    // Check if D3.js is available
    if (!checkD3Availability()) {
        document.getElementById('loading').innerHTML = `<div class="error-message"><i class="fas fa-exclamation-triangle"></i> D3.js is not loaded. Please check your internet connection and reload the page.</div>`;
        return;
    }
    
    // Setup event listeners
    setupEventListeners();
    
    // Initialize the upload feature
    initUploadFeature();
    
    // Initialize the share feature
    initShareFeature();
    
    // Check for shared scenario or show welcome message
    if (!hasSharedData) {
        // Show welcome message instead of loading scenarios
        loadScenarioList();
    }
}

// Start the application when the DOM is loaded
document.addEventListener('DOMContentLoaded', initApp);