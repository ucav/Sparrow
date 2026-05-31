import os
import sys
import requests

def get_follower_count(username: str) -> int:
    bearer_token = os.getenv("TWITTER_BEARER_TOKEN")
    if not bearer_token:
        raise EnvironmentError("Set TWITTER_BEARER_TOKEN env var")
    url = f"https://api.twitter.com/2/users/by/username/{username}"
    params = {"user.fields": "public_metrics"}
    headers = {"Authorization": f"Bearer {bearer_token}"}
    resp = requests.get(url, headers=headers, params=params, timeout=10)
    resp.raise_for_status()
    data = resp.json()
    return data["data"]["public_metrics"]["followers_count"]

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <username>")
        sys.exit(1)
    print(get_follower_count(sys.argv[1]))