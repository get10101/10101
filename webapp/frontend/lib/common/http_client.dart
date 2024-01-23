import 'dart:convert';
import 'package:http/http.dart';

class HttpClientManager {
  static final CustomHttpClient _httpClient = CustomHttpClient(Client(), true);

  static CustomHttpClient get instance => _httpClient;
}

class CustomHttpClient extends BaseClient {
  // TODO: this should come from the settings

  // if this is true, we assume the website is running in dev mode and need to add _host:_port to be able to do http calls
  final bool _dev;

  final String _port = "3001";
  final String _host = "localhost";

  final Client _inner;

  CustomHttpClient(this._inner, this._dev);

  Future<StreamedResponse> send(BaseRequest request) {
    return _inner.send(request);
  }

  @override
  Future<Response> delete(Uri url,
      {Map<String, String>? headers, Object? body, Encoding? encoding}) {
    if (_dev && url.host == '') {
      url = Uri.parse('http://$_host:$_port${url.toString()}');
    }
    return _inner.delete(url, headers: headers, body: body, encoding: encoding);
  }

  @override
  Future<Response> put(Uri url, {Map<String, String>? headers, Object? body, Encoding? encoding}) {
    if (_dev && url.host == '') {
      url = Uri.parse('http://$_host:$_port${url.toString()}');
    }
    return _inner.put(url, headers: headers, body: body, encoding: encoding);
  }

  @override
  Future<Response> post(Uri url, {Map<String, String>? headers, Object? body, Encoding? encoding}) {
    if (_dev && url.host == '') {
      url = Uri.parse('http://$_host:$_port${url.toString()}');
    }
    return _inner.post(url, headers: headers, body: body, encoding: encoding);
  }

  @override
  Future<Response> get(Uri url, {Map<String, String>? headers}) {
    if (_dev && url.host == '') {
      url = Uri.parse('http://$_host:$_port${url.toString()}');
    }
    return _inner.get(url, headers: headers);
  }
}
