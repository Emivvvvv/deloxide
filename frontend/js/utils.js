/**
 * Deloxide - Deadlock Detection Visualization Utilities
 *
 * This file contains utility functions for processing and transforming
 * raw deadlock log data into a format usable by the visualization.
 */

/**
 * Convert raw lock number to string representation
 * Returns the original lock number as a string
 */
function mapLockId(lockNum) {
  return lockNum.toString()
}

/**
 * Transform raw logs into structured log entries
 *
 * @param {Array} rawLogs - Each element is [raw_thread_id, raw_lock, event_code, timestamp]
 * @param {Object} resourceMapping - { raw_lock: letter } e.g. {1: "A", 2: "B"}
 *
 * In logs, thread_id is kept as the original raw value, and timestamp is
 * converted to milliseconds (unix epoch ms) if the raw data is in seconds.
 */
function transformLogs(rawLogs, resourceMapping) {
  // Init log: Information about thread and resource creation at the start of logs
  const rawThreadIds = Array.from(new Set(rawLogs.map((log) => log[0])))
  const allLockLetters = Object.values(resourceMapping)
  const initLog = {
    step: 1,
    timestamp: null, // Set to null to prevent showing timestamp for step 1
    type: "init",
    description: `<span class="thread-id">Threads</span> ${rawThreadIds
      .map((id) => `<span class="thread-id">${id}</span>`)
      .join(
        ", "
      )} <br> <span class="resource-id">Resources</span> ${allLockLetters
      .map((r) => `<span class="resource-id">${r}</span>`)
      .join(", ")}.`,
  }

  const eventTypes = { 0: "attempt", 1: "acquired", 2: "released" }

  // Normal logs: Each thread_id is kept as the original raw value
  // The timestamp value is converted to milliseconds if the raw data is in seconds
  const normalLogs = rawLogs.map((log, idx) => {
    const [rawThread, lockNum, eventCode, timestamp] = log
    return {
      step: idx + 2, // init step is 1, so we start from 2
      timestamp: Math.floor(timestamp * 1000), // Convert to milliseconds
      type: eventTypes[eventCode] || "unknown",
      thread_id: rawThread, // original raw thread id
      resource_id: resourceMapping[lockNum],
      description: `<span class="thread-id">Thread ${rawThread}</span> ${
        eventTypes[eventCode] || "unknown"
      } <span class="resource-id">Resource ${resourceMapping[lockNum]}</span>`,
    }
  })

  // Deadlock log: Collect "attempt" events from normal logs and create a generic description
  let unresolvedEvents = normalLogs.filter((log) => log.type === "attempt")

  // Deduplicate by thread_id and resource_id combination
  // Create a unique key for each thread-resource pair and keep only unique pairs
  const seenPairs = new Map() // Using Map to keep the latest attempt event for each thread-resource pair

  // Process events in reverse order to keep the most recent attempt for each thread-resource pair
  // This ensures we have the most up-to-date state when there are multiple attempts
  for (let i = unresolvedEvents.length - 1; i >= 0; i--) {
    const ev = unresolvedEvents[i]
    const key = `${ev.thread_id}-${ev.resource_id}`
    if (!seenPairs.has(key)) {
      seenPairs.set(key, ev)
    }
  }

  // Convert back to array of unique events
  unresolvedEvents = Array.from(seenPairs.values())

  // Deduplicate thread_id-resource_id pairs that would cause repeated entries in the description
  const threadResourceMapping = new Map()
  for (const ev of unresolvedEvents) {
    if (!threadResourceMapping.has(ev.thread_id)) {
      threadResourceMapping.set(ev.thread_id, ev.resource_id)
    }
  }

  // Recreate unresolvedEvents with only one resource per thread
  unresolvedEvents = Array.from(threadResourceMapping.entries()).map(
    ([thread_id, resource_id]) => ({
      thread_id,
      resource_id,
      type: "attempt",
    })
  )

  // Deadlock cycle: Contains raw thread ids that are waiting
  const deadlockCycle = Array.from(
    new Set(unresolvedEvents.map((ev) => ev.thread_id))
  ).sort()

  // Create a clear description of the deadlock situation
  const deadlockDescriptions = unresolvedEvents.map(
    (ev) =>
      `<span class="thread-id">Thread ${ev.thread_id}</span> is waiting for <span class="resource-id">Resource ${ev.resource_id}</span>`
  )

  // Join descriptions with proper separators for better readability
  let deadlockDescription
  if (deadlockDescriptions.length === 1) {
    deadlockDescription = `<strong>DEADLOCK DETECTED:</strong> ${deadlockDescriptions[0]}`
  } else if (deadlockDescriptions.length === 2) {
    deadlockDescription = `<strong>DEADLOCK DETECTED:</strong> ${
      deadlockDescriptions[0]
    } while ${deadlockDescriptions[1].replace(
      '<span class="thread-id">Thread',
      '<span class="thread-id">Thread'
    )}`
  } else {
    const lastDesc = deadlockDescriptions.pop()
    deadlockDescription = `<strong>DEADLOCK DETECTED:</strong> ${deadlockDescriptions.join(
      ", "
    )} and ${lastDesc.replace(
      '<span class="thread-id">Thread',
      '<span class="thread-id">Thread'
    )}`
  }

  // Last log timestamp: 100 ms after the last event (in milliseconds)
  const lastTimestamp = rawLogs.length
    ? rawLogs[rawLogs.length - 1][3]
    : Date.now() / 1000
  const deadlockTimestamp = Math.floor((lastTimestamp + 0.1) * 1000)
  const deadlockLog = {
    step: normalLogs.length + 2, // After init and normal logs
    timestamp: deadlockTimestamp,
    type: "deadlock",
    cycle: deadlockCycle,
    description: deadlockDescription,
    deadlock_details: {
      thread_cycle: deadlockCycle,
      thread_waiting_for_locks: unresolvedEvents.map((ev) => ({
        thread_id: ev.thread_id,
        lock_id: ev.resource_id,
      })),
      timestamp: deadlockTimestamp,
    },
  }

  return [initLog, ...normalLogs, deadlockLog]
}

/**
 * Generate graph state from logs' cumulative effect
 *
 * @param {Array} logs - Log array created by transformLogs
 * @param {Object} graphThreadMapping - { raw_thread_id: incrementalNum } e.g. {6164146352: 1, 6166292656: 2}
 * @param {Object} resourceMapping - { raw_lock: letter } e.g. {1:"A", 2:"B"}
 *
 * In graph state:
 * - Thread node ids are "T" + mapped number (e.g. "T1", "T2"),
 *   but name field shows the raw thread id value.
 * - Resource nodes are "R" + letter (e.g. "RA", "RB")
 */
function generateGraphStateFromLogs(logs, graphThreadMapping, resourceMapping) {
  const threadNodes = Object.entries(graphThreadMapping)
    .sort((a, b) => a[1] - b[1])
    .map(([rawThread, mapped]) => ({
      id: `T${mapped}`,
      name: `${rawThread.toString()}`,
      type: "thread",
    }))
  const resourceNodes = Object.keys(resourceMapping).map((lockNum) => {
    const letter = resourceMapping[lockNum]
    return {
      id: `R${letter}`,
      name: `Resource ${letter}`,
      type: "resource",
    }
  })
  const nodes = [...threadNodes, ...resourceNodes]

  const graphStates = []
  // Step 1: Initial snapshot with only nodes and no links
  graphStates.push({ step: 1, nodes, links: [] })

  // Cumulative link state: key format is "T{mapped}-R{letter}"
  const cumulativeLinks = {}
  // Normal events: excluding init and deadlock
  const normalEvents = logs.filter(
    (log) => log.type !== "init" && log.type !== "deadlock"
  )
  normalEvents.forEach((ev) => {
    const source = `T${graphThreadMapping[ev.thread_id]}`
    const target = `R${ev.resource_id}`

    // Update link state based on event type
    if (ev.type === "released") {
      // Remove the link when resource is released
      delete cumulativeLinks[`${source}-${target}`]
    } else {
      // Add or update link for attempt or acquired
      cumulativeLinks[`${source}-${target}`] = ev.type
    }

    // Convert current link state to array for D3
    const links = Object.keys(cumulativeLinks).map((key) => {
      const [s, t] = key.split("-")
      return { source: s, target: t, type: cumulativeLinks[key] }
    })

    graphStates.push({
      step: graphStates.length + 1,
      nodes,
      links,
    })
  })

  // Final step: Deadlock graph state snapshot
  const lastSnapshot = graphStates[graphStates.length - 1]
  let deadlockLinks = [...lastSnapshot.links]

  // Add deadlock links connecting threads in cycle
  const deadlockThreads =
    logs.find((log) => log.type === "deadlock")?.cycle || []

  if (deadlockThreads.length >= 2) {
    // Connect all threads in the deadlock cycle
    for (let i = 0; i < deadlockThreads.length; i++) {
      const currentThread = `T${graphThreadMapping[deadlockThreads[i]]}`
      const nextThread = `T${
        graphThreadMapping[deadlockThreads[(i + 1) % deadlockThreads.length]]
      }`
      deadlockLinks.push({
        source: currentThread,
        target: nextThread,
        type: "deadlock",
      })
    }
  } else {
    // Fallback if no clear cycle (should not happen with proper deadlock data)
    const threadIds = Object.values(graphThreadMapping).sort((a, b) => a - b)
    if (threadIds.length >= 2) {
      const t1 = `T${threadIds[0]}`
      const t2 = `T${threadIds[1]}`
      deadlockLinks.push({ source: t1, target: t2, type: "deadlock" })
      deadlockLinks.push({ source: t2, target: t1, type: "deadlock" })
    }
  }

  graphStates.push({
    step: graphStates.length + 1,
    nodes,
    links: deadlockLinks,
  })

  return graphStates
}

/**
 * Transform raw object into logs and graph_state arrays
 *
 * rawData: [ rawLogs, rawGraph ]
 * Here rawGraph is not used; the graph state is derived from logs
 */
function transformRawObject(rawData) {
  const rawLogs = rawData[0]

  // For graph: Generate incremental numbers for raw thread ids
  const graphThreadMapping = {}
  let nextGraphThread = 1
  rawLogs.forEach((log) => {
    const rawThread = log[0]
    if (!(rawThread in graphThreadMapping)) {
      graphThreadMapping[rawThread] = nextGraphThread++
    }
  })

  // Resource mapping: Convert raw lock number to letter
  const resourceMapping = {}
  rawLogs.forEach((log) => {
    const lockNum = log[1]
    if (!(lockNum in resourceMapping)) {
      resourceMapping[lockNum] = mapLockId(lockNum)
    }
  })

  // Thread_ids in logs are stored as raw values
  const logs = transformLogs(rawLogs, resourceMapping)
  const graph_state = generateGraphStateFromLogs(
    logs,
    graphThreadMapping,
    resourceMapping
  )

  return { logs, graph_state }
}

/**
 * Decode logs from URL-safe Base64, Gzip and MessagePack encoded string
 */
function decodeLogs(encodedStr) {
  var base64 = encodedStr.replace(/-/g, "+").replace(/_/g, "/")
  var binaryStr = atob(base64)
  var len = binaryStr.length
  var bytes = new Uint8Array(len)
  for (var i = 0; i < len; i++) {
    bytes[i] = binaryStr.charCodeAt(i)
  }
  var decompressed = pako.ungzip(bytes)
  var logsData = msgpack.decode(decompressed)
  return logsData
}

/**
 * Process encoded log from URL and return transformed data
 */
function processEncodedLog(encodedStr) {
  const decoded = decodeLogs(encodedStr)
  return transformRawObject(decoded)
}

/**
 * Process logs in the new format (one JSON object per line)
 *
 * @param {string} logText - Raw text containing one JSON object per line
 * @returns {Object} - Structured logs and graph state data
 */
function processNewFormatLogs(logText) {
  try {
    // Parse the log text into an array of event objects
    const lines = logText.trim().split("\n")
    const events = lines.map((line) => JSON.parse(line))

    // Get all unique thread IDs and lock IDs from the events
    const allThreads = new Set()
    const allLocks = new Set()

    events.forEach((event) => {
      if (event.event && event.event.thread_id) {
        allThreads.add(event.event.thread_id)
      }
      if (event.event && event.event.lock_id) {
        allLocks.add(event.event.lock_id)
      }
    })

    // Create resource mapping (lock_id -> letter)
    const resourceMapping = {}
    Array.from(allLocks)
      .sort()
      .forEach((lockId, index) => {
        resourceMapping[lockId] = mapLockId(index + 1)
      })

    // Create thread mapping for the graph (raw_thread_id -> incremental number)
    const graphThreadMapping = {}
    Array.from(allThreads)
      .sort()
      .forEach((threadId, index) => {
        graphThreadMapping[threadId] = index + 1
      })

    // Build structured logs array
    const logs = []

    // Add init log as first step
    const threadsStr = Array.from(allThreads).join(", ")
    const resourcesStr = Object.values(resourceMapping)
      .map((r) => `Resource ${r}`)
      .join(", ")

    logs.push({
      step: 1,
      timestamp: null,
      type: "init",
      description: `<span style="color:black">Created:</span> <br> <span class="thread-id">Threads</span> ${threadsStr}</span> <br> <span class="resource-id">Resources</span> ${resourcesStr}.`,
    })

    // Process each event
    events.forEach((eventObj, idx) => {
      const { event } = eventObj
      if (!event) return

      // Map event names
      const eventTypeMap = {
        Attempt: "attempt",
        Acquired: "acquired",
        Released: "released",
      }

      const eventType = eventTypeMap[event.event] || "unknown"

      // Skip if not a valid event type
      if (eventType === "unknown") return

      const threadId = event.thread_id
      const resourceId = resourceMapping[event.lock_id]
      const timestamp = Math.floor(event.timestamp * 1000) // Convert to milliseconds

      logs.push({
        step: idx + 2, // init step is 1, so we start from 2
        timestamp,
        type: eventType,
        thread_id: threadId,
        resource_id: resourceId,
        description: `<span class="thread-id">Thread ${threadId}</span> ${eventType} <span class="resource-id">Resource ${resourceId}</span>`,
      })
    })

    // Check for deadlock by looking at the last graph state
    const lastEventWithGraph = events[events.length - 1]

    if (lastEventWithGraph && lastEventWithGraph.graph) {
      const graph = lastEventWithGraph.graph
      const attemptLinks = graph.links.filter((link) => link.type === "Attempt")

      // If we have multiple threads attempting to acquire resources, check for a deadlock
      if (attemptLinks.length >= 2) {
        // Find threads that are both attempting to acquire a resource and already hold one
        const threadsInDeadlock = []
        const waitingDetails = []

        graph.threads.forEach((threadId) => {
          const acquiredResources = graph.links
            .filter(
              (link) => link.source === threadId && link.type === "Acquired"
            )
            .map((link) => link.target)

          const attemptedResources = graph.links
            .filter(
              (link) => link.source === threadId && link.type === "Attempt"
            )
            .map((link) => link.target)

          if (acquiredResources.length > 0 && attemptedResources.length > 0) {
            threadsInDeadlock.push(threadId)
            attemptedResources.forEach((resourceId) => {
              waitingDetails.push({
                thread_id: threadId,
                lock_id: resourceMapping[resourceId],
              })
            })
          }
        })

        // If we found threads in a potential deadlock
        if (threadsInDeadlock.length >= 2) {
          // Create descriptions
          const deadlockDescriptions = waitingDetails.map(
            (detail) =>
              `<span class="thread-id">Thread ${detail.thread_id}</span> is waiting for <span class="resource-id">Resource ${detail.lock_id}</span>`
          )

          // Join descriptions with proper separators for better readability
          let deadlockDescription
          if (deadlockDescriptions.length === 1) {
            deadlockDescription = `<strong>DEADLOCK DETECTED:</strong> ${deadlockDescriptions[0]}`
          } else if (deadlockDescriptions.length === 2) {
            deadlockDescription = `<strong>DEADLOCK DETECTED:</strong> ${
              deadlockDescriptions[0]
            } while ${deadlockDescriptions[1].replace(
              '<span class="thread-id">Thread',
              '<span class="thread-id">Thread'
            )}`
          } else {
            const lastDesc = deadlockDescriptions.pop()
            deadlockDescription = `<strong>DEADLOCK DETECTED:</strong> ${deadlockDescriptions.join(
              ", "
            )} and ${lastDesc.replace(
              '<span class="thread-id">Thread',
              '<span class="thread-id">Thread'
            )}`
          }

          // Create deadlock log
          const lastTimestamp = events[events.length - 1].event.timestamp
          const deadlockTimestamp = Math.floor((lastTimestamp + 0.1) * 1000)

          logs.push({
            step: logs.length + 1,
            timestamp: deadlockTimestamp,
            type: "deadlock",
            cycle: threadsInDeadlock,
            description: deadlockDescription,
            deadlock_details: {
              thread_cycle: threadsInDeadlock,
              thread_waiting_for_locks: waitingDetails,
              timestamp: deadlockTimestamp,
            },
          })
        }
      }
    }

    // Generate graph states based on logs
    const graphStates = generateGraphStateFromLogs(
      logs,
      graphThreadMapping,
      resourceMapping
    )

    return {
      logs,
      graph_state: graphStates,
    }
  } catch (error) {
    console.error("Error processing new format logs:", error)
    throw error
  }
}
