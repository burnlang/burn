package interpreter

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/burnlang/burn/pkg/ast"
)

var httpHeaders = map[string]string{
	"User-Agent": "BurnLang/1.0",
	"Accept":     "application/json",
}

func (i *Interpreter) registerHTTPLibrary() {
	
	i.types["HTTPResponse"] = &ast.TypeDefinition{
		Name: "HTTPResponse",
		Fields: []ast.TypeField{
			{Name: "statusCode", Type: "int"},
			{Name: "body", Type: "string"},
			{Name: "headers", Type: "array"},
		},
	}

	httpClass := NewClass("HTTP")

	httpClass.AddStatic("get", &ast.FunctionDeclaration{
		Name:       "get",
		Parameters: []ast.Parameter{{Name: "url", Type: "string"}},
		ReturnType: "HTTPResponse",
	})
	httpClass.AddStatic("post", &ast.FunctionDeclaration{
		Name:       "post",
		Parameters: []ast.Parameter{{Name: "url", Type: "string"}, {Name: "body", Type: "string"}},
		ReturnType: "HTTPResponse",
	})
	httpClass.AddStatic("put", &ast.FunctionDeclaration{
		Name:       "put",
		Parameters: []ast.Parameter{{Name: "url", Type: "string"}, {Name: "body", Type: "string"}},
		ReturnType: "HTTPResponse",
	})
	httpClass.AddStatic("delete", &ast.FunctionDeclaration{
		Name:       "delete",
		Parameters: []ast.Parameter{{Name: "url", Type: "string"}},
		ReturnType: "HTTPResponse",
	})
	httpClass.AddStatic("getHeader", &ast.FunctionDeclaration{
		Name:       "getHeader",
		Parameters: []ast.Parameter{{Name: "response", Type: "HTTPResponse"}, {Name: "name", Type: "string"}},
		ReturnType: "string",
	})
	httpClass.AddStatic("parseJSON", &ast.FunctionDeclaration{
		Name:       "parseJSON",
		Parameters: []ast.Parameter{{Name: "body", Type: "string"}},
		ReturnType: "any",
	})
	httpClass.AddStatic("setHeaders", &ast.FunctionDeclaration{
		Name:       "setHeaders",
		Parameters: []ast.Parameter{{Name: "headers", Type: "array"}},
		ReturnType: "bool",
	})

	i.classes["HTTP"] = httpClass
	i.environment["HTTP"] = httpClass

	
	i.environment["HTTP.get"] = &BuiltinFunction{
		Name: "HTTP.get",
		Fn:   i.httpGet,
	}
	i.environment["HTTP.post"] = &BuiltinFunction{
		Name: "HTTP.post",
		Fn:   i.httpPost,
	}
	i.environment["HTTP.put"] = &BuiltinFunction{
		Name: "HTTP.put",
		Fn:   i.httpPut,
	}
	i.environment["HTTP.delete"] = &BuiltinFunction{
		Name: "HTTP.delete",
		Fn:   i.httpDelete,
	}
	i.environment["HTTP.getHeader"] = &BuiltinFunction{
		Name: "HTTP.getHeader",
		Fn:   i.httpGetHeader,
	}
	i.environment["HTTP.parseJSON"] = &BuiltinFunction{
		Name: "HTTP.parseJSON",
		Fn:   i.httpParseJSON,
	}
	i.environment["HTTP.setHeaders"] = &BuiltinFunction{
		Name: "HTTP.setHeaders",
		Fn:   i.httpSetHeaders,
	}

	
	i.environment["get"] = i.environment["HTTP.get"]
	i.environment["post"] = i.environment["HTTP.post"]
	i.environment["put"] = i.environment["HTTP.put"]
	i.environment["delete"] = i.environment["HTTP.delete"]
	i.environment["getHeader"] = i.environment["HTTP.getHeader"]
	i.environment["parseJSON"] = i.environment["HTTP.parseJSON"]
	i.environment["setHeaders"] = i.environment["HTTP.setHeaders"]
}

func (i *Interpreter) httpGet(args []Value) (Value, error) {
	if len(args) != 1 {
		return nil, fmt.Errorf("HTTP.get expects exactly one string argument")
	}
	urlStr, ok := args[0].(string)
	if !ok {
		return nil, fmt.Errorf("HTTP.get expects a string URL")
	}

	client := &http.Client{Timeout: time.Second * 30}
	req, err := http.NewRequest("GET", urlStr, nil)
	if err != nil {
		return nil, fmt.Errorf("error creating request: %v", err)
	}

	for k, v := range httpHeaders {
		req.Header.Add(k, v)
	}

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("error making request: %v", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("error reading response: %v", err)
	}

	headers := []Value{}
	for name, values := range resp.Header {
		for _, value := range values {
			headers = append(headers, fmt.Sprintf("%s: %s", name, value))
		}
	}

	return &Struct{
		TypeName: "HTTPResponse",
		Fields: map[string]interface{}{
			"statusCode": resp.StatusCode,
			"body":       string(body),
			"headers":    headers,
		},
	}, nil
}

func (i *Interpreter) httpPost(args []Value) (Value, error) {
	if len(args) != 2 {
		return nil, fmt.Errorf("HTTP.post expects exactly two string arguments (url, body)")
	}
	urlStr, ok := args[0].(string)
	if !ok {
		return nil, fmt.Errorf("HTTP.post expects a string URL as first argument")
	}
	bodyStr, ok := args[1].(string)
	if !ok {
		return nil, fmt.Errorf("HTTP.post expects a string body as second argument")
	}

	client := &http.Client{Timeout: time.Second * 30}
	req, err := http.NewRequest("POST", urlStr, strings.NewReader(bodyStr))
	if err != nil {
		return nil, fmt.Errorf("error creating request: %v", err)
	}

	for k, v := range httpHeaders {
		req.Header.Add(k, v)
	}

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("error making request: %v", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("error reading response: %v", err)
	}

	headers := []Value{}
	for name, values := range resp.Header {
		for _, value := range values {
			headers = append(headers, fmt.Sprintf("%s: %s", name, value))
		}
	}

	return &Struct{
		TypeName: "HTTPResponse",
		Fields: map[string]interface{}{
			"statusCode": resp.StatusCode,
			"body":       string(body),
			"headers":    headers,
		},
	}, nil
}

func (i *Interpreter) httpPut(args []Value) (Value, error) {
	if len(args) != 2 {
		return nil, fmt.Errorf("HTTP.put expects exactly two string arguments (url, body)")
	}
	urlStr, ok := args[0].(string)
	if !ok {
		return nil, fmt.Errorf("HTTP.put expects a string URL as first argument")
	}
	bodyStr, ok := args[1].(string)
	if !ok {
		return nil, fmt.Errorf("HTTP.put expects a string body as second argument")
	}

	client := &http.Client{Timeout: time.Second * 30}
	req, err := http.NewRequest("PUT", urlStr, strings.NewReader(bodyStr))
	if err != nil {
		return nil, fmt.Errorf("error creating request: %v", err)
	}

	for k, v := range httpHeaders {
		req.Header.Add(k, v)
	}

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("error making request: %v", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("error reading response: %v", err)
	}

	headers := []Value{}
	for name, values := range resp.Header {
		for _, value := range values {
			headers = append(headers, fmt.Sprintf("%s: %s", name, value))
		}
	}

	return &Struct{
		TypeName: "HTTPResponse",
		Fields: map[string]interface{}{
			"statusCode": resp.StatusCode,
			"body":       string(body),
			"headers":    headers,
		},
	}, nil
}

func (i *Interpreter) httpDelete(args []Value) (Value, error) {
	if len(args) != 1 {
		return nil, fmt.Errorf("HTTP.delete expects exactly one string argument")
	}
	urlStr, ok := args[0].(string)
	if !ok {
		return nil, fmt.Errorf("HTTP.delete expects a string URL")
	}

	client := &http.Client{Timeout: time.Second * 30}
	req, err := http.NewRequest("DELETE", urlStr, nil)
	if err != nil {
		return nil, fmt.Errorf("error creating request: %v", err)
	}

	for k, v := range httpHeaders {
		req.Header.Add(k, v)
	}

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("error making request: %v", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("error reading response: %v", err)
	}

	headers := []Value{}
	for name, values := range resp.Header {
		for _, value := range values {
			headers = append(headers, fmt.Sprintf("%s: %s", name, value))
		}
	}

	return &Struct{
		TypeName: "HTTPResponse",
		Fields: map[string]interface{}{
			"statusCode": resp.StatusCode,
			"body":       string(body),
			"headers":    headers,
		},
	}, nil
}

func (i *Interpreter) httpSetHeaders(args []Value) (Value, error) {
	if len(args) != 1 {
		return nil, fmt.Errorf("HTTP.setHeaders expects exactly one array argument")
	}
	headerArray, ok := args[0].([]Value)
	if !ok {
		return nil, fmt.Errorf("HTTP.setHeaders expects an array of header strings")
	}

	newHeaders := make(map[string]string)
	for _, hv := range headerArray {
		headerStr, ok := hv.(string)
		if !ok {
			return nil, fmt.Errorf("each header must be a string")
		}
		parts := strings.SplitN(headerStr, ":", 2)
		if len(parts) != 2 {
			return nil, fmt.Errorf("invalid header format: %s", headerStr)
		}
		name := strings.TrimSpace(parts[0])
		value := strings.TrimSpace(parts[1])
		newHeaders[name] = value
	}

	httpHeaders = newHeaders
	return true, nil
}

func (i *Interpreter) httpGetHeader(args []Value) (Value, error) {
	if len(args) != 2 {
		return nil, fmt.Errorf("HTTP.getHeader expects exactly two arguments")
	}
	respObj, ok := args[0].(*Struct)
	if !ok || respObj.TypeName != "HTTPResponse" {
		return nil, fmt.Errorf("HTTP.getHeader expects an HTTPResponse as first argument")
	}
	headerName, ok := args[1].(string)
	if !ok {
		return nil, fmt.Errorf("HTTP.getHeader expects a string header name")
	}

	headers, ok := respObj.Fields["headers"].([]Value)
	if !ok {
		return "", nil
	}

	headerName = strings.ToLower(headerName)
	for _, h := range headers {
		headerStr, ok := h.(string)
		if !ok {
			continue
		}
		parts := strings.SplitN(headerStr, ":", 2)
		if len(parts) != 2 {
			continue
		}
		name := strings.ToLower(strings.TrimSpace(parts[0]))
		value := strings.TrimSpace(parts[1])
		if name == headerName {
			return value, nil
		}
	}
	return "", nil
}

func (i *Interpreter) httpParseJSON(args []Value) (Value, error) {
	if len(args) != 1 {
		return nil, fmt.Errorf("HTTP.parseJSON expects exactly one string argument")
	}
	jsonStr, ok := args[0].(string)
	if !ok {
		return nil, fmt.Errorf("HTTP.parseJSON expects a string JSON")
	}

	var result interface{}
	err := json.Unmarshal([]byte(jsonStr), &result)
	if err != nil {
		return nil, fmt.Errorf("error parsing JSON: %v", err)
	}

	return convertJSONToBurn(result), nil
}

func convertJSONToBurn(value interface{}) Value {
	switch v := value.(type) {
	case map[string]interface{}:
		fields := make(map[string]interface{})
		for key, val := range v {
			fields[key] = convertJSONToBurn(val)
		}
		return &Struct{
			TypeName: "Object",
			Fields:   fields,
		}
	case []interface{}:
		array := make([]Value, len(v))
		for i, val := range v {
			array[i] = convertJSONToBurn(val)
		}
		return array
	case string:
		return v
	case float64:
		return v
	case bool:
		return v
	case nil:
		return nil
	default:
		return fmt.Sprintf("%v", v)
	}
}
