import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/util/environment.dart';
import 'package:http/http.dart' as http;
import 'package:html/parser.dart' as html;

class MemeService {
  final String _url;

  MemeService() : _url = _setUrl();

  static String _setUrl() {
    bridge.Config config = Environment.parse();
    var memeEndpoint = config.memeEndpoint;
    if (!memeEndpoint.endsWith('/')) {
      memeEndpoint += '/';
    }

    return memeEndpoint;
  }

  Future<List<Meme>> fetchMemeImages() async {
    final response = await http.get(Uri.parse(_url));

    if (response.statusCode == 200) {
      var document = html.parse(response.body);

      // Extract all <a> tags with href attributes besides `..`
      List<Meme> memes = document
          .querySelectorAll('a[href]')
          .map((element) => element.attributes['href']!)
          .where((href) => !href.contains('..'))
          .map((item) => Meme.fromJson(_url, item))
          .toList();
      return memes;
    } else {
      throw Exception('Failed to load meme images');
    }
  }
}

class Meme {
  final String downloadUrl;

  Meme({
    required this.downloadUrl,
  });

  factory Meme.fromJson(String url, String name) {
    return Meme(
      downloadUrl: "$url/$name",
    );
  }

  @override
  String toString() {
    return downloadUrl;
  }
}
