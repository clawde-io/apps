/// JSON-RPC 2.0 envelope types.
library;

/// A JSON-RPC 2.0 request.
class RpcRequest {
  final String jsonrpc = '2.0';
  final String method;
  final Map<String, dynamic>? params;
  final dynamic id;

  const RpcRequest({required this.method, this.params, this.id});

  Map<String, dynamic> toJson() => {
        'jsonrpc': jsonrpc,
        'method': method,
        if (params != null) 'params': params,
        if (id != null) 'id': id,
      };
}

/// A JSON-RPC 2.0 response.
class RpcResponse {
  final String jsonrpc;
  final dynamic result;
  final RpcError? error;
  final dynamic id;

  const RpcResponse({
    required this.jsonrpc,
    this.result,
    this.error,
    this.id,
  });

  bool get isError => error != null;

  factory RpcResponse.fromJson(Map<String, dynamic> json) => RpcResponse(
        jsonrpc: json['jsonrpc'] as String,
        result: json['result'],
        error: json['error'] != null
            ? RpcError.fromJson(json['error'] as Map<String, dynamic>)
            : null,
        id: json['id'],
      );
}

/// A JSON-RPC 2.0 error object.
class RpcError {
  final int code;
  final String message;
  final dynamic data;

  const RpcError({required this.code, required this.message, this.data});

  factory RpcError.fromJson(Map<String, dynamic> json) => RpcError(
        code: json['code'] as int,
        message: json['message'] as String,
        data: json['data'],
      );

  @override
  String toString() => 'RpcError($code): $message';
}

/// Standard clawd error codes.
abstract final class ClawdError {
  static const int sessionNotFound = -32001;
  static const int providerNotAvailable = -32002;
  static const int rateLimited = -32003;
  static const int unauthorized = -32004;
  static const int repoNotFound = -32005;
  static const int sessionPaused = -32006;
}
