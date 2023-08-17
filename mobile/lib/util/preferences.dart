import 'package:shared_preferences/shared_preferences.dart';

enum Network { regtest, mainnet }

class Preferences {
  Preferences._privateConstructor();

  static final Preferences instance = Preferences._privateConstructor();

  static const userSeedBackupConfirmed = "userSeedBackupConfirmed";
  static const emailAddress = "emailAddress";
  static const network = "network";

  setUserSeedBackupConfirmed() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setBool(userSeedBackupConfirmed, true);
  }

  Future<bool> isUserSeedBackupConfirmed() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getBool(userSeedBackupConfirmed) ?? false;
  }

  setEmailAddress(String value) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(emailAddress, value);
  }

  Future<String> getEmailAddress() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getString(emailAddress) ?? "";
  }

  setNetwork(Network value) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(network, value.toString());
  }

  Future<Network> getNetwork() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    final networkString = preferences.getString(network) ?? "";
    if (networkString == Network.mainnet.toString()) {
      return Network.mainnet;
    } else {
      return Network.regtest;
    }
  }

  Future<bool> hasEmailAddress() async {
    var email = await getEmailAddress();
    return email.isNotEmpty;
  }
}
