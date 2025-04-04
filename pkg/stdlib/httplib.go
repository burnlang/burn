package stdlib

// HTTPLib contains the source code for the HTTP standard library
const HTTPLib = `// Burn Standard Library: HTTP Module
// This provides HTTP request functionality for Burn programs

// HTTPResponse struct to represent an HTTP response
def HTTPResponse {
    statusCode: int,
    body: string,
    headers: [string]
}

// HTTP class with static methods for making HTTP requests
class HTTP {
    // Make a GET request to the specified URL
    static fun get(url: string): HTTPResponse {
        // Implementation provided by interpreter
    }
    
    // Make a POST request to the specified URL with the given body
    static fun post(url: string, body: string): HTTPResponse {
        // Implementation provided by interpreter
    }
    
    // Make a PUT request to the specified URL with the given body
    static fun put(url: string, body: string): HTTPResponse {
        // Implementation provided by interpreter
    }
    
    // Make a DELETE request to the specified URL
    static fun delete(url: string): HTTPResponse {
        // Implementation provided by interpreter
    }
    
    // Set HTTP headers for subsequent requests
    static fun setHeaders(headers: [string]): bool {
        // Implementation provided by interpreter
    }
    
    // Get a specific header from an HTTP response
    static fun getHeader(response: HTTPResponse, name: string): string {
        // Implementation provided by interpreter
    }
    
    // Parse JSON from a string into Burn data structures
    static fun parseJSON(body: string): any {
        // Implementation provided by interpreter
    }
}
`
