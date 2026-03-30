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

	return map[string]interface{}{
		"statusCode": float64(resp.StatusCode),
		"body":       string(body),
		"headers":    headers,
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

	return map[string]interface{}{
		"statusCode": float64(resp.StatusCode),
		"body":       string(body),
		"headers":    headers,
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

	return map[string]interface{}{
		"statusCode": float64(resp.StatusCode),
		"body":       string(body),
		"headers":    headers,
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

	return map[string]interface{}{
		"statusCode": float64(resp.StatusCode),
		"body":       string(body),
		"headers":    headers,
	}, nil
}

func (i *Interpreter) httpSetHeaders(args []Value) (Value, error) {
	if len(args) != 1 {
		return nil, fmt.Errorf("HTTP.setHeaders expects exactly one array argument")
	}

	headers, ok := args[0].([]Value)
	if !ok {
		return nil, fmt.Errorf("HTTP.setHeaders expects an array of strings")
	}

	httpHeaders = make(map[string]string)

	for _, header := range headers {
		headerStr, ok := header.(string)
		if !ok {
			return nil, fmt.Errorf("HTTP.setHeaders expects an array of strings")
		}

		parts := strings.SplitN(headerStr, ":", 2)
		if len(parts) != 2 {
			return nil, fmt.Errorf("Invalid header format: %s", headerStr)
		}

		key := strings.TrimSpace(parts[0])
		value := strings.TrimSpace(parts[1])
		httpHeaders[key] = value
	}

	return true, nil
}

func (i *Interpreter) httpGetHeader(args []Value) (Value, error) {
	if len(args) != 2 {
		return nil, fmt.Errorf("HTTP.getHeader expects exactly two arguments (response, headerName)")
	}

	response, ok := args[0].(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("HTTP.getHeader expects an HTTPResponse as first argument")
	}

	headerName, ok := args[1].(string)
	if !ok {
		return nil, fmt.Errorf("HTTP.getHeader expects a string header name as second argument")
	}

	headers, ok := response["headers"].([]Value)
	if !ok {
		return "", nil
	}

	for _, header := range headers {
		headerStr, ok := header.(string)
		if !ok {
			continue
		}

		parts := strings.SplitN(headerStr, ":", 2)
		if len(parts) == 2 {
			key := strings.TrimSpace(parts[0])
			value := strings.TrimSpace(parts[1])
			if strings.ToLower(key) == strings.ToLower(headerName) {
				return value, nil
			}
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
	case nil:
		return nil
	case bool:
		return v
	case float64:
		return v
	case string:
		return v
	case []interface{}:
		arr := make([]Value, len(v))
		for i, item := range v {
			arr[i] = convertJSONToBurn(item)
		}
		return arr
	case map[string]interface{}:
		obj := make(map[string]interface{})
		for key, val := range v {
			obj[key] = convertJSONToBurn(val)
		}
		return obj
	default:
		return fmt.Sprintf("%v", v)
	}
}
