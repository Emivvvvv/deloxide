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
let scenarioList = [];
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
    try {
        const response = await fetch('data/scenarios.json');
        if (!response.ok) {
            throw new Error(`Failed to load scenarios: ${response.status} ${response.statusText}`);
        }
        
        const data = await response.json();
        scenarioList = data.scenarios;
        
        // Create scenario selector dropdown
        const scenarioContainer = document.createElement('div');
        scenarioContainer.id = 'scenario-selector';
        scenarioContainer.className = 'scenario-selector';
        
        const scenarioLabel = document.createElement('label');
        scenarioLabel.textContent = 'Select Scenario:';
        scenarioLabel.htmlFor = 'scenario-dropdown';
        
        const scenarioDropdown = document.createElement('select');
        scenarioDropdown.id = 'scenario-dropdown';
        
        scenarioList.forEach(scenario => {
            const option = document.createElement('option');
            option.value = scenario.id;
            option.textContent = scenario.title;
            scenarioDropdown.appendChild(option);
        });
        
        scenarioContainer.appendChild(scenarioLabel);
        scenarioContainer.appendChild(scenarioDropdown);
        
        // Insert before controls
        const controlsElement = document.getElementById('controls');
        controlsElement.parentNode.insertBefore(scenarioContainer, controlsElement);
        
        // Add event listener for scenario change
        scenarioDropdown.addEventListener('change', (event) => {
            const selectedScenarioId = event.target.value;
            const selectedScenario = scenarioList.find(s => s.id === selectedScenarioId);
            
            if (selectedScenario) {
                loadScenario(selectedScenario.file);
            }
        });
        
        // Load the first scenario by default
        if (scenarioList.length > 0) {
            loadScenario(scenarioList[0].file);
        }
    } catch (error) {
        console.error('Error loading scenario list:', error);
        document.getElementById('loading').innerHTML = `
            <div class="error-message">
                <i class="fas fa-exclamation-triangle"></i> 
                Error loading scenario list: ${error.message}. Please check your network connection and try reloading the page.
            </div>`;
    }
}

/**
 * Load a specific scenario by filename
 */
async function loadScenario(filename) {
    try {
        // Reset visualization
        resetVisualization();
        
        // Show loading state
        document.getElementById('loading').style.display = 'block';
        document.getElementById('graph').style.display = 'none';
        document.getElementById('step-info').style.display = 'none';
        document.getElementById('wait-graph').style.display = 'none';
        document.getElementById('timeline').style.display = 'none';
        
        const response = await fetch(`data/${filename}`);
        if (!response.ok) {
            throw new Error(`Failed to load scenario: ${response.status} ${response.statusText}`);
        }
        
        const scenarioData = await response.json();
        currentScenario = scenarioData;
        logData = scenarioData.logs;
        graphStateData = scenarioData.graph_state;
        
        // Update scenario description
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
        
        if (existingInfo) {
            mainElement.replaceChild(scenarioInfo, existingInfo);
        } else {
            const scenarioSelector = document.getElementById('scenario-selector');
            mainElement.insertBefore(scenarioInfo, scenarioSelector.nextSibling);
        }
        
        // Initialize visualization
        currentStep = 1;
        initVisualization();
        
        // Hide loading message and show visualization elements
        document.getElementById('loading').style.display = 'none';
        document.getElementById('graph').style.display = 'block';
        document.getElementById('step-info').style.display = 'block';
        document.getElementById('timeline').style.display = 'block';
        
        // Initialize timeline
        initTimeline();
        
        // Update visualization for the first step
        updateVisualization();
    } catch (error) {
        console.error('Error loading scenario:', error);
        document.getElementById('loading').innerHTML = `
            <div class="error-message">
                <i class="fas fa-exclamation-triangle"></i> 
                Error loading scenario: ${error.message}. Please try selecting a different scenario or reload the page.
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
    
    // Initialize theme
    initTheme();
    
    // Check if D3.js is available
    if (!checkD3Availability()) {
        document.getElementById('loading').innerHTML = `<div class="error-message"><i class="fas fa-exclamation-triangle"></i> D3.js is not loaded. Please check your internet connection and reload the page.</div>`;
        return;
    }
    
    // Load scenario list
    loadScenarioList();
    
    // Set up event listeners
    setupEventListeners();
}

// Start the application when the DOM is loaded
document.addEventListener('DOMContentLoaded', initApp);