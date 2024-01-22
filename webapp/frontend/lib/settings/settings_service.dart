import 'package:http/http.dart' as http;

class SettingsService {
  const SettingsService();

  Future<String> getNodeId() async {
    // TODO(holzeis): this should come from the config
    const port = "3001";
    const host = "localhost";

    try {
      final response = await http.get(Uri.http('$host:$port', '/api/node'));

      if (response.statusCode == 200) {
        return response.body;
      } else {
        return "unknown";
      }
    } catch (e) {
      return "unknown";
    }
  }
}
