// Test script to validate the line-by-line JSON format parser

// Sample log in the line-by-line format
const sampleLog = `{"event":{"thread_id":0,"lock_id":1,"event":"Spawn","timestamp":1745845779.018466,"parent_id":2},"graph":{"threads":[],"locks":[1],"links":[]}}
{"event":{"thread_id":0,"lock_id":2,"event":"Spawn","timestamp":1745845779.018894,"parent_id":2},"graph":{"threads":[],"locks":[1,2],"links":[]}}
{"event":{"thread_id":0,"lock_id":3,"event":"Spawn","timestamp":1745845779.01891,"parent_id":2},"graph":{"threads":[],"locks":[3,1,2],"links":[]}}
{"event":{"thread_id":3,"lock_id":0,"event":"Spawn","timestamp":1745845779.018992,"parent_id":2},"graph":{"threads":[3],"locks":[6,5,2,3,1,4],"links":[]}}
{"event":{"thread_id":3,"lock_id":6,"event":"Attempt","timestamp":1745845779.019026},"graph":{"threads":[3],"locks":[6,5,2,3,1,4],"links":[{"source":3,"target":6,"type":"Attempt"}]}}`;

// Mock function for processNewFormatLogs
function processNewFormatLogs(logText) {
  try {
    let jsonData;
    
    // Check if the input is already a parsed object or a string that needs parsing
    if (typeof logText === 'string') {
      // Handle the line-by-line JSON format
      if (logText.trim().startsWith("{") && logText.includes('{"event":')) {
        // Split by newlines and parse each line as a separate JSON object
        const lines = logText.trim().split('\n');
        const events = [];
        const graphState = [];
        
        // Process each line as a separate JSON object
        for (const line of lines) {
          if (!line.trim()) continue; // Skip empty lines
          
          try {
            const lineData = JSON.parse(line.trim());
            if (lineData.event) {
              const { thread_id, lock_id, event, timestamp, parent_id } = lineData.event;
              
              // Convert event to event code
              let eventCode;
              switch (event) {
                case 'Attempt': eventCode = 0; break;
                case 'Acquired': eventCode = 1; break;
                case 'Released': eventCode = 2; break;
                case 'Spawn': eventCode = 3; break;
                case 'Exit': eventCode = 4; break;
                default: eventCode = -1; // Unknown event
              }
              
              // Create event in the expected format [thread_id, lock_id, event_code, timestamp, parent_id]
              const formattedEvent = [
                thread_id, 
                lock_id, 
                eventCode, 
                timestamp,
                parent_id || 0
              ];
              
              events.push(formattedEvent);
              
              // Also store the graph state if available
              if (lineData.graph) {
                graphState.push(lineData.graph);
              }
            }
            console.log(`Successfully parsed line: ${line.substring(0, 50)}...`);
          } catch (lineError) {
            console.error("Error parsing JSON line:", lineError, line);
            // Continue with next line instead of failing completely
          }
        }
        
        console.log(`Processed ${events.length} events from ${lines.length} lines`);
        // Create the data structure expected by the rest of the code
        jsonData = { events, graphs: graphState };
        
        // Just return the processed events for testing
        return jsonData;
      } else {
        // Try to parse as a single JSON object
        jsonData = JSON.parse(logText);
      }
    } else {
      jsonData = logText;
    }
    
    // Return the parsed data
    return jsonData;
  } catch (error) {
    console.error("Error processing logs:", error);
    throw error;
  }
}

// Test the parser
try {
  console.log("Testing line-by-line JSON parser with sample log...");
  const result = processNewFormatLogs(sampleLog);
  console.log("Success! Processed events:", result.events.length);
  console.log("First event:", result.events[0]);
  console.log("Event codes correctly mapped:", 
    result.events.map(evt => evt[2]).join(", "));
} catch (error) {
  console.error("Test failed:", error);
} 